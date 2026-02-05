pub mod api;
pub mod assets;
pub mod auth;
pub mod cleanup;
pub mod cli;
pub mod db;
pub mod jwt;
pub mod names;
pub mod rate_limit;

// Re-export test-mode utilities for easier access in tests
#[cfg(feature = "test-mode")]
pub use cli::local_ip_extractor;

use api::create_api_router;
use assets::{AssetsState, app_handler, dashboard_handler, login_handler, login_index_handler};
use auth::add_access_token_cookie;
use axum::{Router, middleware, response::Redirect, routing::get};
use db::Database;
use jwt::JwtConfig;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use url::Url;
use webauthn_rs::prelude::*;

pub struct ServerConfig {
    /// Base path for the application (e.g., "/app" or "/crow-base")
    pub base: Option<String>,
    /// Database connection (cloneable, uses connection pool internally)
    pub db: Database,
    /// WebAuthn relying party ID (domain name)
    pub rp_id: String,
    /// WebAuthn relying party origin (full URL)
    pub rp_origin: Url,
    /// JWT secret for signing tokens
    pub jwt_secret: Vec<u8>,
    /// Whether to set Secure flag on cookies (should be true in production with HTTPS)
    pub secure_cookies: bool,
    /// Whether new user signups are disabled
    pub no_signup: bool,
    /// Whether to add a random nonce to CSP headers for each HTML response
    pub csp_nonce: bool,
    /// IP extraction strategy (requires running behind a proxy)
    pub ip_extractor: Option<cli::IpExtractor>,
}

/// Create the application router with the given configuration.
pub fn create_app(config: &ServerConfig) -> Router {
    // Create JWT config
    let jwt = Arc::new(JwtConfig::new(&config.jwt_secret));

    // Build assets state (handles all frontend config internally)
    let state = AssetsState::new(
        config.base.as_deref(),
        config.csp_nonce,
        jwt.clone(),
        config.db.clone(),
        config.secure_cookies,
        config.ip_extractor.clone(),
    )
    .expect("Failed to initialize assets");

    // Create WebAuthn instance
    let webauthn = Arc::new(
        WebauthnBuilder::new(&config.rp_id, &config.rp_origin)
            .expect("Failed to create WebAuthn builder")
            .rp_name("Crowchiper")
            .build()
            .expect("Failed to build WebAuthn"),
    );

    let api_router = create_api_router(
        config.db.clone(),
        webauthn,
        jwt.clone(),
        config.secure_cookies,
        config.no_signup,
        config.ip_extractor.clone(),
    )
    .layer(middleware::from_fn(add_access_token_cookie));

    // Get paths from state
    let login_path = state.login_path();
    let app_path = state.app_path();
    let dashboard_path = state.dashboard_path();

    // Login assets (public, no auth)
    // Index routes redirect authenticated users to the app
    let login_routes = Router::new()
        .route(login_path, get(login_index_handler))
        .route(&format!("{}/", login_path), get(login_index_handler))
        .route(
            &format!("{}/index.html", login_path),
            get(login_index_handler),
        )
        .route(&format!("{}/{{*path}}", login_path), get(login_handler))
        .with_state(state.clone());

    // App assets (protected, JWT required)
    let app_routes = Router::new()
        .route(app_path, get(app_handler))
        .route(&format!("{}/", app_path), get(app_handler))
        .route(&format!("{}/{{*path}}", app_path), get(app_handler))
        .with_state(state.clone())
        .layer(middleware::from_fn(add_access_token_cookie));

    // Dashboard assets (protected, JWT required)
    let dashboard_routes = Router::new()
        .route(dashboard_path, get(dashboard_handler))
        .route(&format!("{}/", dashboard_path), get(dashboard_handler))
        .route(
            &format!("{}/{{*path}}", dashboard_path),
            get(dashboard_handler),
        )
        .with_state(state.clone())
        .layer(middleware::from_fn(add_access_token_cookie));

    let base_path = config.base.as_deref().unwrap_or("");
    let redirect_path: &'static str = if base_path.is_empty() {
        "/"
    } else {
        Box::leak(base_path.to_string().into_boxed_str())
    };

    Router::new()
        .route(redirect_path, get(Redirect::temporary(login_path)))
        .nest(state.api_path, api_router)
        .merge(login_routes)
        .merge(app_routes)
        .merge(dashboard_routes)
}

/// Run cleanup tasks and spawn background scheduler.
/// Call this before starting the server.
pub async fn init_cleanup(db: &Database) {
    cleanup::run_cleanup(db).await;
    cleanup::spawn_cleanup_scheduler(db.clone());
}

/// Run the server on the given listener. This function blocks until the server exits.
/// Call `init_cleanup` before this to run cleanup on startup.
pub async fn run_server(config: ServerConfig, listener: TcpListener) -> Result<(), std::io::Error> {
    let app = create_app(&config);
    let make_service = app.into_make_service_with_connect_info::<SocketAddr>();
    axum::serve(listener, make_service).await
}

/// Start the server on the given port in a background task. Use port 0 to let the OS choose a random port.
/// Returns the actual address the server is listening on.
/// Note: For production use, prefer `run_server` directly in main.
pub async fn start_server(
    config: ServerConfig,
    port: u16,
) -> (tokio::task::JoinHandle<()>, SocketAddr) {
    // Run cleanup tasks on startup
    init_cleanup(&config.db).await;

    let addr = format!("127.0.0.1:{}", port);
    let listener = TcpListener::bind(&addr).await.expect("Failed to bind");
    let local_addr = listener.local_addr().expect("Failed to get local address");

    let handle = tokio::spawn(async move {
        run_server(config, listener).await.ok();
    });

    (handle, local_addr)
}
