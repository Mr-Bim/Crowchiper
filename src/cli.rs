//! CLI argument parsing, validation, and startup helpers.

use crate::ServerConfig;
use crate::db::Database;
use crate::names::generate_name;
use crate::plugin::{PluginManager, PluginRuntime, PluginSpec, parse_plugin_spec};
use clap::Parser;
use tracing::{error, info};
use url::Url;
use uuid::Uuid;

const MIN_JWT_SECRET_LENGTH: usize = 32;

#[derive(clap::ValueEnum, Clone, Debug, Default)]
pub enum LogFormat {
    #[default]
    Pretty,
    Json,
    Compact,
}

#[derive(clap::ValueEnum, Clone, Debug)]
pub enum ClientIpHeader {
    CFConnectingIP,
    XRealIp,
    XForwardFor,
    Forward,
    #[cfg(feature = "test-mode")]
    Local,
}

/// IP extraction strategy resolved at startup. Stores header name and parsing function.
#[derive(Clone)]
pub struct IpExtractor {
    pub header_name: &'static str,
    parse_fn: fn(&str) -> Result<String, &'static str>,
}

impl std::fmt::Debug for IpExtractor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IpExtractor")
            .field("header_name", &self.header_name)
            .finish()
    }
}

impl IpExtractor {
    /// Extract IP from the header value using the configured parsing strategy.
    pub fn extract(&self, header_value: &str) -> Result<String, &'static str> {
        (self.parse_fn)(header_value)
    }
}

impl From<ClientIpHeader> for IpExtractor {
    fn from(header: ClientIpHeader) -> Self {
        match header {
            ClientIpHeader::CFConnectingIP => IpExtractor {
                header_name: "cf-connecting-ip",
                parse_fn: parse_single_ip,
            },
            ClientIpHeader::XRealIp => IpExtractor {
                header_name: "x-real-ip",
                parse_fn: parse_single_ip,
            },
            ClientIpHeader::XForwardFor => IpExtractor {
                header_name: "x-forwarded-for",
                parse_fn: parse_x_forwarded_for,
            },
            ClientIpHeader::Forward => IpExtractor {
                header_name: "forwarded",
                parse_fn: parse_forwarded,
            },
            #[cfg(feature = "test-mode")]
            ClientIpHeader::Local => IpExtractor {
                header_name: "",
                parse_fn: |_| Ok("127.0.0.1".to_string()),
            },
        }
    }
}

/// Create a local IP extractor for test mode that always returns 127.0.0.1.
#[cfg(feature = "test-mode")]
pub fn local_ip_extractor() -> IpExtractor {
    IpExtractor::from(ClientIpHeader::Local)
}

/// Validate that a string is a valid IP address (IPv4 or IPv6).
fn validate_ip(ip: &str) -> Result<String, &'static str> {
    ip.parse::<std::net::IpAddr>()
        .map(|addr| addr.to_string())
        .map_err(|_| "IP header contains invalid IP address")
}

/// Parse a single IP value (CF-Connecting-IP, X-Real-IP).
fn parse_single_ip(value: &str) -> Result<String, &'static str> {
    let ip = value.trim();
    if ip.is_empty() {
        return Err("IP header is empty");
    }
    validate_ip(ip)
}

/// Parse X-Forwarded-For header (comma-separated list, take first).
fn parse_x_forwarded_for(value: &str) -> Result<String, &'static str> {
    let ip = value
        .split(',')
        .next()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .ok_or("X-Forwarded-For header has no valid IP")?;
    validate_ip(ip)
}

/// Parse RFC 7239 Forwarded header.
fn parse_forwarded(value: &str) -> Result<String, &'static str> {
    for part in value.split(',') {
        for param in part.split(';') {
            let param = param.trim();
            if let Some(ip_value) = param.strip_prefix("for=") {
                let ip = ip_value.trim().trim_matches('"');
                // Handle IPv6 in brackets: [2001:db8::1]
                let ip = ip.trim_start_matches('[').trim_end_matches(']');
                // Remove port if present (e.g., "192.0.2.60:8080" or "[2001:db8::1]:8080")
                let ip = if let Some(colon_pos) = ip.rfind(':') {
                    // Check if this is IPv6 without brackets (contains multiple colons)
                    if ip.matches(':').count() > 1 {
                        ip // IPv6 address, keep as-is
                    } else {
                        &ip[..colon_pos] // IPv4 with port, strip port
                    }
                } else {
                    ip
                };
                if !ip.is_empty() {
                    return validate_ip(ip);
                }
            }
        }
    }
    Err("Forwarded header has no valid 'for' parameter")
}

#[derive(Parser, Debug, Clone)]
#[command(
    name = "Crowchiper",
    about = "Personal posts with passkey authentication",
    after_help = "ENVIRONMENT VARIABLES:\n    JWT_SECRET    JWT signing secret (required, min 32 chars)"
)]
pub struct Args {
    /// Base path for reverse proxy (e.g., /app)
    #[arg(short, long, value_parser = validate_base_path)]
    pub base: Option<String>,

    /// Port to listen on
    #[arg(short, long, default_value = "7291")]
    pub port: u16,

    /// SQLite database path
    #[arg(short, long, default_value = "crowchiper.db")]
    pub database: String,

    /// WebAuthn Relying Party ID (domain)
    #[arg(long, default_value = "localhost")]
    pub rp_id: String,

    /// WebAuthn origin URL (must be HTTPS for non-localhost)
    #[arg(long, default_value = "http://localhost:7291")]
    pub rp_origin: String,

    /// Read JWT secret from file instead of JWT_SECRET env var
    #[arg(long)]
    pub jwt_secret_file: Option<String>,

    /// Create admin user and print claim URL
    #[arg(long)]
    pub create_admin: bool,

    /// Disable public registration
    #[arg(long)]
    pub no_signup: bool,

    /// Add random nonce to CSP headers
    #[arg(long)]
    pub csp_nonce: bool,

    /// Log format
    #[arg(short, long, default_value = "pretty", value_enum)]
    pub log_format: LogFormat,

    /// Extract client IP from header (requires reverse proxy)
    #[arg(short, long, value_enum)]
    pub ip_header: Option<ClientIpHeader>,

    /// WASM plugin. See README for details. Format: path.wasm[:net,env-VAR,fs-read=/p,fs-write=/p,var-k=v]
    #[arg(long, value_parser = parse_plugin_spec)]
    pub plugin: Vec<PluginSpec>,

    /// Behavior when a plugin fails to load: abort (default) or warn
    #[arg(long, default_value = "abort", value_enum)]
    pub plugin_error: PluginErrorMode,
}

#[derive(clap::ValueEnum, Clone, Debug, Default)]
pub enum PluginErrorMode {
    /// Abort startup if a plugin fails to load
    #[default]
    Abort,
    /// Log a warning and continue without the plugin
    Warn,
}

fn validate_base_path(s: &str) -> Result<String, String> {
    if s.is_empty() {
        return Ok(String::new());
    }

    if !s.starts_with('/') {
        return Err(format!("Base path must start with '/': {}", s));
    }

    if s.len() > 1 && s.ends_with('/') {
        return Err(format!("Base path must not end with '/': {}", s));
    }

    if s.chars().any(|c| !c.is_ascii() || c.is_whitespace()) {
        return Err(format!("Base path contains invalid characters: {}", s));
    }

    Ok(s.to_string())
}

/// Initialize logging based on the specified format.
pub fn init_logging(format: &LogFormat) {
    match format {
        LogFormat::Pretty => tracing_subscriber::fmt::init(),
        LogFormat::Json => tracing_subscriber::fmt().json().init(),
        LogFormat::Compact => tracing_subscriber::fmt().compact().init(),
    }
}

/// Load JWT secret from environment variable or file.
/// Returns None and logs an error if the secret cannot be loaded.
pub fn load_jwt_secret(jwt_secret_file: Option<&str>) -> Option<String> {
    let secret = if let Ok(secret) = std::env::var("JWT_SECRET") {
        // Clear the environment variable to prevent leaking
        // SAFETY: We're single-threaded at this point during startup,
        // and no other code is reading this environment variable.
        unsafe { std::env::remove_var("JWT_SECRET") };
        secret
    } else if let Some(path) = jwt_secret_file {
        match std::fs::read_to_string(path) {
            Ok(content) => content.trim().to_string(),
            Err(e) => {
                error!(path = %path, error = %e, "Failed to read JWT secret file");
                return None;
            }
        }
    } else {
        error!(
            "JWT secret is required. Set JWT_SECRET environment variable (recommended) or use --jwt-secret-file"
        );
        return None;
    };

    if secret.len() < MIN_JWT_SECRET_LENGTH {
        error!(
            "JWT secret is shorter than {} characters. Use a longer secret",
            MIN_JWT_SECRET_LENGTH
        );
        return None;
    }

    Some(secret)
}

/// Parse and validate the rp-origin URL.
/// Returns None and logs an error if validation fails.
pub fn validate_rp_origin(rp_origin: &str) -> Option<Url> {
    let url = match Url::parse(rp_origin) {
        Ok(url) => url,
        Err(e) => {
            error!(origin = %rp_origin, error = %e, "Invalid rp-origin URL");
            return None;
        }
    };

    let is_https = url.scheme() == "https";
    let is_localhost = url.host_str() == Some("localhost");

    if !is_https && !is_localhost {
        error!("rp-origin must use HTTPS for non-localhost deployments");
        return None;
    }

    Some(url)
}

/// Handle the --create-admin flag: create a new admin or show existing pending admin.
pub async fn handle_create_admin(db: &Database, rp_origin: &str, base: Option<&str>) {
    let base = base.unwrap_or("");
    let login_path = env!("CONFIG_LOGIN_ASSETS");

    match db.users().get_pending_admin().await {
        Ok(Some(existing)) => {
            let claim_url = format!(
                "{}{}{}/claim.html?uuid={}",
                rp_origin, base, login_path, existing.uuid
            );
            println!();
            println!("Pending admin already exists: {}", existing.username);
            println!("Claim URL: {}", claim_url);
            println!();
        }
        Ok(None) => {
            let uuid = Uuid::new_v4().to_string();
            let username = generate_name();

            match db.users().create_admin(&uuid, &username).await {
                Ok(_) => {
                    let claim_url = format!(
                        "{}{}{}/claim.html?uuid={}",
                        rp_origin, base, login_path, uuid
                    );
                    println!();
                    println!("Admin user created: {}", username);
                    println!("Claim URL: {}", claim_url);
                    println!();
                }
                Err(e) => {
                    error!(error = %e, "Failed to create admin user");
                    std::process::exit(1);
                }
            }
        }
        Err(e) => {
            error!(error = %e, "Failed to check for existing admin");
            std::process::exit(1);
        }
    }
}

/// Build ServerConfig from validated arguments.
pub fn build_config(
    base: Option<String>,
    db: Database,
    rp_id: String,
    rp_origin: Url,
    jwt_secret: String,
    no_signup: bool,
    csp_nonce: bool,
    ip_header: Option<ClientIpHeader>,
    plugins: Vec<PluginRuntime>,
) -> ServerConfig {
    let secure_cookies = rp_origin.scheme() == "https";

    let plugin_manager = if plugins.is_empty() {
        None
    } else {
        Some(std::sync::Arc::new(PluginManager::new(plugins)))
    };

    ServerConfig {
        base,
        db,
        rp_id,
        rp_origin,
        jwt_secret: jwt_secret.into_bytes(),
        secure_cookies,
        no_signup,
        csp_nonce,
        ip_extractor: ip_header.map(IpExtractor::from),
        plugin_manager,
    }
}

/// Open the database, logging errors if it fails.
pub async fn open_database(path: &str) -> Option<Database> {
    match Database::open(path).await {
        Ok(db) => {
            info!(path = %path, "Database opened");
            Some(db)
        }
        Err(e) => {
            error!(path = %path, error = %e, "Failed to open database");
            None
        }
    }
}
