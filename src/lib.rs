pub mod api;
pub mod assets;
pub mod auth;
pub mod cli;
pub mod db;
pub mod jwt;
pub mod names;

use api::create_api_router;
use assets::{
    AssetsState, app_handler, app_handler_direct, login_handler, login_handler_direct,
    login_index_handler, process_app_html_files, process_login_html_files,
};

use auth::NEW_ACCESS_TOKEN_COOKIE;
use axum::{
    Router,
    http::header::SET_COOKIE,
    middleware::{self, Next},
    response::{Redirect, Response},
    routing::get,
};
use db::Database;
use jwt::JwtConfig;
use std::cell::RefCell;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tracing::info;
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
    /// Header to extract client IP from (requires running behind a proxy)
    pub ip_header: Option<cli::ClientIpHeader>,
}

/// Leak a String to get a &'static str. Used for paths that live for the program lifetime.
fn leak_string(s: String) -> &'static str {
    Box::leak(s.into_boxed_str())
}

/// Middleware to add the new access token cookie to responses when token was refreshed.
async fn add_access_token_cookie(request: axum::extract::Request, next: Next) -> Response {
    // Set up task-local scope and run the handler inside it
    NEW_ACCESS_TOKEN_COOKIE
        .scope(RefCell::new(None), async {
            let mut response = next.run(request).await;

            // Check if a new access token cookie was set by the auth extractor
            if let Some(cookie) = NEW_ACCESS_TOKEN_COOKIE.with(|cell| cell.borrow_mut().take()) {
                if let Ok(value) = cookie.parse() {
                    response.headers_mut().append(SET_COOKIE, value);
                }
            }

            response
        })
        .await
}

/// Create the application router with the given configuration.
pub fn create_app(config: &ServerConfig) -> Router {
    let base_path: &'static str = leak_string(config.base.clone().unwrap_or("".to_string()));
    let api_path: &'static str = leak_string(AssetsState::make_api_path(base_path));
    let login_path: &'static str = leak_string(AssetsState::make_login_path(base_path));
    let app_path: &'static str = leak_string(AssetsState::make_app_path(base_path));

    // Process HTML files for path rewriting when base is set
    let (login_assets_html, app_assets_html, login_assets_handler, app_assets_handler) =
        if base_path.is_empty() {
            (
                HashMap::default(),
                HashMap::default(),
                get(login_handler_direct),
                get(app_handler_direct),
            )
        } else {
            (
                process_login_html_files(login_path),
                process_app_html_files(app_path),
                get(login_handler),
                get(app_handler),
            )
        };

    // Create WebAuthn instance
    let webauthn = Arc::new(
        WebauthnBuilder::new(&config.rp_id, &config.rp_origin)
            .expect("Failed to create WebAuthn builder")
            .rp_name("Crowchiper")
            .build()
            .expect("Failed to build WebAuthn"),
    );

    // Create JWT config
    let jwt = Arc::new(JwtConfig::new(&config.jwt_secret));

    let state = AssetsState::new(
        api_path,
        login_path,
        app_path,
        login_assets_html,
        app_assets_html,
        jwt.clone(),
        config.db.clone(),
        config.secure_cookies,
        config.csp_nonce,
        config.ip_header.clone(),
    )
    .expect("Failed to initialize assets");

    let api_router = create_api_router(
        config.db.clone(),
        webauthn,
        jwt.clone(),
        config.secure_cookies,
        config.no_signup,
        config.ip_header.clone(),
    )
    .layer(middleware::from_fn(add_access_token_cookie));

    // Login assets (public, no auth)
    // Index routes redirect authenticated users to the app
    let login_routes = Router::new()
        .route(login_path, get(login_index_handler))
        .route(&format!("{}/", login_path), get(login_index_handler))
        .route(
            &format!("{}/index.html", login_path),
            get(login_index_handler),
        )
        .route(&format!("{}/{{*path}}", login_path), login_assets_handler)
        .with_state(state.clone());

    // App assets (protected, JWT required)
    let app_routes = Router::new()
        .route(app_path, app_assets_handler.clone())
        .route(&format!("{}/", app_path), app_assets_handler.clone())
        .route(&format!("{}/{{*path}}", app_path), app_assets_handler)
        .with_state(state)
        .layer(middleware::from_fn(add_access_token_cookie));

    let redirect_path: &'static str = if base_path.is_empty() { "/" } else { base_path };

    Router::new()
        .route(redirect_path, get(Redirect::temporary(login_path)))
        .nest(api_path, api_router)
        .merge(login_routes)
        .merge(app_routes)
}

/// Start the server on the given port. Use port 0 to let the OS choose a random port.
/// Returns the actual address the server is listening on.
pub async fn start_server(
    config: ServerConfig,
    port: u16,
) -> (tokio::task::JoinHandle<()>, SocketAddr) {
    // Clean up expired tokens on startup
    match config.db.tokens().delete_expired().await {
        Ok(count) if count > 0 => info!("Cleaned up {} expired tokens", count),
        Ok(_) => {}
        Err(e) => tracing::warn!("Failed to clean up expired tokens: {}", e),
    }

    let app = create_app(&config);
    let addr = format!("127.0.0.1:{}", port);
    let listener = TcpListener::bind(&addr).await.expect("Failed to bind");
    let local_addr = listener.local_addr().expect("Failed to get local address");

    let handle = tokio::spawn(async move {
        // Use a custom service that injects ConnectInfo as Extension
        let make_service = app.into_make_service_with_connect_info::<SocketAddr>();

        // Serve with our custom make_service
        axum::serve(listener, make_service).await.ok();
    });

    (handle, local_addr)
}
