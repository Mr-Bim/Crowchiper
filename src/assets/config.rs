use std::collections::HashMap;
use std::sync::Arc;

use rust_embed::Embed;
use tracing::info;

use crate::auth::{HasAssetAuthBackend, ServerSettings};
use crate::db::Database;
use crate::impl_has_auth_backend;
use crate::jwt::JwtConfig;

use super::csp::{APP_CSP_HEADER, DASHBOARD_CSP_HEADER, LOGIN_CSP_HEADER};
use super::embed::{AppAssets, DashboardAssets, LoginAssets};
use super::response::{
    HtmlResponder, html_response_static, html_response_with_nonce, serve_asset,
    serve_html_processed, serve_html_raw,
};

/// Processed HTML files map type
pub(super) type ProcessedHtmlMap = Arc<HashMap<&'static str, &'static str>>;

/// Function signature for frontend serving.
/// Selected at startup based on whether processed HTML exists.
type FrontendServer = fn(&str, &FrontendConfig, HtmlResponder) -> axum::response::Response;

/// Configuration for a single frontend (login, app, dashboard, etc.)
#[derive(Clone)]
pub struct FrontendConfig {
    /// URL path for this frontend (e.g., "/login", "/fiery-sparrow")
    pub path: &'static str,
    /// CSP header for HTML responses
    pub csp_header: &'static str,
    /// Pre-processed HTML files (for base path rewriting). Empty if no rewriting needed.
    processed_html: ProcessedHtmlMap,
    /// Function to serve this frontend (selected at startup based on base path)
    pub(super) server: FrontendServer,
}

/// Generate frontend server functions for a given asset type.
/// Creates `serve_{name}_raw`, `serve_{name}_processed`, and `{name}_server` functions.
macro_rules! frontend_server {
    ($name:ident, $assets:ty) => {
        paste::paste! {
            fn [<serve_ $name _raw>](
                path: &str,
                config: &FrontendConfig,
                html_responder: HtmlResponder,
            ) -> axum::response::Response {
                if path.ends_with(".html") {
                    return serve_html_raw::<$assets>(path, config.csp_header, html_responder);
                }
                serve_asset::<$assets>(path)
            }

            fn [<serve_ $name _processed>](
                path: &str,
                config: &FrontendConfig,
                html_responder: HtmlResponder,
            ) -> axum::response::Response {
                if path.ends_with(".html") {
                    return serve_html_processed(
                        path,
                        &config.processed_html,
                        config.csp_header,
                        html_responder,
                    );
                }
                serve_asset::<$assets>(path)
            }

            fn [<$name _server>](has_processed: bool) -> FrontendServer {
                if has_processed {
                    [<serve_ $name _processed>]
                } else {
                    [<serve_ $name _raw>]
                }
            }
        }
    };
}

frontend_server!(login, LoginAssets);
frontend_server!(app, AppAssets);
frontend_server!(dashboard, DashboardAssets);

/// The login assets path, set at compile time
const CONFIG_LOGIN_ASSETS: &str = env!("CONFIG_LOGIN_ASSETS");

/// The app assets path from config.json, set at compile time
const CONFIG_APP_ASSETS: &str = env!("CONFIG_APP_ASSETS");

/// The dashboard assets path, set at compile time
const CONFIG_DASHBOARD_ASSETS: &str = env!("CONFIG_DASHBOARD_ASSETS");

fn process_html_files<T: Embed>(
    frontend_path: &str,
    config_assets: &str,
) -> HashMap<&'static str, &'static str> {
    let mut processed_html = HashMap::new();
    let src_pattern = format!(r#"src="{}/"#, config_assets);
    let href_pattern = format!(r#"href="{}/"#, config_assets);

    for file in T::iter() {
        if file.ends_with(".html") {
            if let Some(content) = T::get(&file) {
                let html = String::from_utf8_lossy(&content.data);
                let rewritten = html
                    .replace(&src_pattern, &format!(r#"src="{}/"#, frontend_path))
                    .replace(&href_pattern, &format!(r#"href="{}/"#, frontend_path));
                let leaked: &'static str = Box::leak(rewritten.into_boxed_str());
                let file_key: &'static str = Box::leak(file.to_string().into_boxed_str());
                processed_html.insert(file_key, leaked);
            }
        }
    }

    info!(count = processed_html.len(), base = %frontend_path, "Rewrote HTML files");

    processed_html
}

/// Leak a String to get a &'static str. Used for paths that live for the program lifetime.
fn leak_string(s: String) -> &'static str {
    Box::leak(s.into_boxed_str())
}

/// Build a FrontendConfig for a given frontend type.
macro_rules! build_frontend {
    ($name:ident, $assets:ty, $csp:expr, $config_assets:expr, $base_path:expr) => {{
        let path: &'static str = leak_string(format!("{}{}", $base_path, $config_assets));
        let has_base_path = !$base_path.is_empty();
        let processed_html = ProcessedHtmlMap::new(if has_base_path {
            process_html_files::<$assets>(path, $config_assets)
        } else {
            HashMap::default()
        });
        FrontendConfig {
            path,
            csp_header: $csp,
            processed_html,
            server: $name(has_base_path),
        }
    }};
}

#[derive(Clone)]
pub struct AssetsState {
    pub api_path: &'static str,
    /// Function to generate HTML responses (with or without nonce)
    pub(super) html_responder: HtmlResponder,
    /// Login frontend config
    pub(super) login: FrontendConfig,
    /// App frontend config
    pub(super) app: FrontendConfig,
    /// Dashboard frontend config
    pub(super) dashboard: FrontendConfig,
    /// Pre-resolved login index.html content
    pub(super) login_index_html: &'static str,
    pub jwt: Arc<JwtConfig>,
    pub db: Database,
    pub settings: ServerSettings,
}

impl_has_auth_backend!(AssetsState);

impl HasAssetAuthBackend for AssetsState {
    fn login_path(&self) -> &str {
        self.login.path
    }
}

impl AssetsState {
    /// Create a new AssetsState with all frontend configurations.
    ///
    /// # Arguments
    /// * `base_path` - Optional base path prefix (e.g., "/app" for reverse proxy)
    /// * `csp_nonce` - Whether to add random nonces to CSP headers
    /// * `jwt` - JWT configuration for token validation
    /// * `db` - Database connection
    ///
    /// # Returns
    /// Returns an error if login/index.html is missing from embedded assets.
    pub fn new(
        base_path: Option<&str>,
        csp_nonce: bool,
        jwt: Arc<JwtConfig>,
        db: Database,
        settings: ServerSettings,
    ) -> Result<Self, &'static str> {
        let base: &'static str = leak_string(base_path.unwrap_or("").to_string());
        let api_path: &'static str = leak_string(format!("{}/api", base));

        // Choose HTML responder based on CSP nonce setting
        let html_responder = if csp_nonce {
            html_response_with_nonce
        } else {
            html_response_static
        };

        // Build frontend configs
        let login = build_frontend!(
            login_server,
            LoginAssets,
            LOGIN_CSP_HEADER,
            CONFIG_LOGIN_ASSETS,
            base
        );
        let app = build_frontend!(
            app_server,
            AppAssets,
            APP_CSP_HEADER,
            CONFIG_APP_ASSETS,
            base
        );
        let dashboard = build_frontend!(
            dashboard_server,
            DashboardAssets,
            DASHBOARD_CSP_HEADER,
            CONFIG_DASHBOARD_ASSETS,
            base
        );

        // Get login index HTML - use processed version if available, otherwise raw
        let login_index_html = if let Some(&html) = login.processed_html.get("index.html") {
            html
        } else {
            let content = LoginAssets::get("index.html")
                .ok_or("login/index.html missing from embedded assets")?;
            let html = String::from_utf8_lossy(&content.data);
            Box::leak(html.into_owned().into_boxed_str())
        };

        Ok(Self {
            api_path,
            html_responder,
            login,
            app,
            dashboard,
            login_index_html,
            jwt,
            db,
            settings,
        })
    }

    /// Get the login frontend path (e.g., "/login")
    pub fn login_path(&self) -> &'static str {
        self.login.path
    }

    /// Get the app frontend path (e.g., "/fiery-sparrow")
    pub fn app_path(&self) -> &'static str {
        self.app.path
    }

    /// Get the dashboard frontend path (e.g., "/dashboard")
    pub fn dashboard_path(&self) -> &'static str {
        self.dashboard.path
    }
}
