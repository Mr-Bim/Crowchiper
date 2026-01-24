//! CLI argument parsing, validation, and startup helpers.

use crate::ServerConfig;
use crate::db::Database;
use crate::names::generate_name;
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

#[derive(Parser, Debug, Clone)]
#[command(
    name = "Crowchiper",
    about = "Personal posts with passkey authentication"
)]
pub struct Args {
    #[arg(short, long, value_parser = validate_base_path,
        help = format!("Base path prefix. Login at {{base}}/login, app at {{base}}{}", env!("CONFIG_APP_ASSETS")))]
    pub base: Option<String>,

    /// Port to listen on
    #[arg(short, long, default_value = "7291")]
    pub port: u16,

    /// Path to SQLite database file
    #[arg(short, long, default_value = "crowchiper.db")]
    pub database: String,

    /// WebAuthn Relying Party ID (domain name, e.g., "localhost" or "example.com")
    #[arg(long, default_value = "localhost")]
    pub rp_id: String,

    /// WebAuthn Relying Party Origin (full URL, e.g., "http://localhost:7291")
    #[arg(long, default_value = "http://localhost:7291")]
    pub rp_origin: String,

    /// Path to file containing JWT secret. Prefer using JWT_SECRET env var instead
    #[arg(long)]
    pub jwt_secret_file: Option<String>,

    /// Create a new admin user on startup and print the claim URL
    #[arg(long)]
    pub create_admin: bool,

    /// Disable new user signups (admin creation via --create-admin still works)
    #[arg(long)]
    pub no_signup: bool,

    /// Add a random nonce to CSP headers for each HTML response, does not affect script tags
    #[arg(long)]
    pub csp_nonce: bool,

    /// Log output format
    #[arg(short, long, default_value = "pretty")]
    pub log_format: LogFormat,
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
) -> ServerConfig {
    let secure_cookies = rp_origin.scheme() == "https";

    ServerConfig {
        base,
        db,
        rp_id,
        rp_origin,
        jwt_secret: jwt_secret.into_bytes(),
        secure_cookies,
        no_signup,
        csp_nonce,
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
