use axum::{
    extract::FromRequestParts,
    http::{StatusCode, header, request::Parts},
    response::{IntoResponse, Redirect, Response},
};
use rust_embed::Embed;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::info;

use crate::auth::{ACCESS_COOKIE_NAME, REFRESH_COOKIE_NAME, get_cookie};
use crate::jwt::JwtConfig;

/// Login assets (public, no auth required)
#[derive(Embed)]
#[folder = "dist/login/"]
pub struct LoginAssets;

/// App assets (protected, JWT required)
#[derive(Embed)]
#[folder = "dist/app/"]
pub struct AppAssets;

#[derive(Clone)]
pub struct AssetsState {
    pub api_path: &'static str,
    pub login_path: &'static str,
    pub app_path: &'static str,
    /// Pre-resolved login index.html content (rewritten if base path is set)
    pub login_index_html: &'static str,
    pub processed_login_html: HashMap<&'static str, &'static str>,
    pub processed_app_html: HashMap<&'static str, &'static str>,
    pub jwt: Arc<JwtConfig>,
}

impl AssetsState {
    /// Create a new AssetsState.
    /// Returns an error if login/index.html is missing from assets.
    pub fn new(
        api_path: &'static str,
        login_path: &'static str,
        app_path: &'static str,
        processed_login_html: HashMap<&'static str, &'static str>,
        processed_app_html: HashMap<&'static str, &'static str>,
        jwt: Arc<JwtConfig>,
    ) -> Result<Self, &'static str> {
        // Get login index HTML - use processed version if available, otherwise raw
        let login_index_html = if let Some(&html) = processed_login_html.get("index.html") {
            html
        } else {
            let content = LoginAssets::get("index.html")
                .ok_or("login/index.html missing from embedded assets")?;
            let html = String::from_utf8_lossy(&content.data);
            Box::leak(html.into_owned().into_boxed_str())
        };

        Ok(Self {
            api_path,
            login_path,
            app_path,
            login_index_html,
            processed_login_html,
            processed_app_html,
            jwt,
        })
    }

    pub fn make_api_path(base: &str) -> String {
        format!("{}/api", base)
    }

    pub fn make_login_path(base: &str) -> String {
        format!("{}{}", base, env!("CONFIG_LOGIN_ASSETS"))
    }

    pub fn make_app_path(base: &str) -> String {
        format!("{}{}", base, env!("CONFIG_APP_ASSETS"))
    }
}

// =============================================================================
// Asset Auth (redirects to login on failure)
// =============================================================================

/// Extractor that validates JWT from cookie for asset routes.
/// Returns the claims if valid, or redirects to login if invalid.
pub struct RequireAuth;

/// Authentication errors for asset routes - redirects to login.
#[derive(Debug)]
pub enum AuthError {
    MissingToken { login_path: &'static str },
    InvalidToken { login_path: &'static str },
}

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        let login_path = match self {
            AuthError::MissingToken { login_path } | AuthError::InvalidToken { login_path } => {
                login_path
            }
        };
        Redirect::temporary(login_path).into_response()
    }
}

impl FromRequestParts<Arc<AssetsState>> for RequireAuth {
    type Rejection = AuthError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &Arc<AssetsState>,
    ) -> Result<Self, Self::Rejection> {
        let login_path = state.login_path;

        // Try access token first, then refresh token
        if let Some(token) = get_cookie(&parts.headers, ACCESS_COOKIE_NAME) {
            if state.jwt.validate_access_token(token).is_ok() {
                return Ok(RequireAuth);
            }
        }

        if let Some(token) = get_cookie(&parts.headers, REFRESH_COOKIE_NAME) {
            if state.jwt.validate_refresh_token(token).is_ok() {
                return Ok(RequireAuth);
            }
        }

        Err(AuthError::MissingToken { login_path })
    }
}

// =============================================================================
// HTML Processing
// =============================================================================

/// The login assets path, set at compile time
const CONFIG_LOGIN_ASSETS: &str = env!("CONFIG_LOGIN_ASSETS");

/// The app assets path from config.json, set at compile time
const CONFIG_APP_ASSETS: &str = env!("CONFIG_APP_ASSETS");

/// Process HTML files to rewrite asset URLs when a custom base path is used.
pub fn process_login_html_files(login_path: &str) -> HashMap<&'static str, &'static str> {
    process_html_files::<LoginAssets>(login_path, CONFIG_LOGIN_ASSETS)
}

/// Process app HTML files to rewrite asset URLs when a custom base path is used.
pub fn process_app_html_files(app_path: &str) -> HashMap<&'static str, &'static str> {
    process_html_files::<AppAssets>(app_path, CONFIG_APP_ASSETS)
}

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

// =============================================================================
// Asset Handlers
// =============================================================================

#[inline]
fn normalize_path(path: Option<&axum::extract::Path<String>>) -> &str {
    match path.map(|p| p.as_str()) {
        Some(p) if !p.is_empty() => p,
        _ => "index.html",
    }
}

/// Cache duration for immutable hashed assets (1 year)
const IMMUTABLE_CACHE: &str = "public, max-age=31536000, immutable";
/// Cache duration for HTML files (no cache, always revalidate)
const NO_CACHE: &str = "no-cache";

#[inline]
fn serve_asset<T: Embed>(path: &str) -> Response {
    match T::get(path) {
        Some(content) => {
            let mime = mime_guess::from_path(path).first_or_octet_stream();
            // Hashed assets in /assets/ are immutable, HTML files should not be cached
            let cache_control = if path.starts_with("assets/") {
                IMMUTABLE_CACHE
            } else {
                NO_CACHE
            };
            (
                [
                    (header::CONTENT_TYPE, mime.as_ref()),
                    (header::CACHE_CONTROL, cache_control),
                ],
                content.data,
            )
                .into_response()
        }
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

/// Serve login assets directly (no base path rewriting)
pub async fn login_handler_direct(path: Option<axum::extract::Path<String>>) -> Response {
    serve_asset::<LoginAssets>(normalize_path(path.as_ref()))
}

/// Serve login index page, redirecting authenticated users to the app.
pub async fn login_index_handler(
    axum::extract::State(state): axum::extract::State<Arc<AssetsState>>,
    headers: axum::http::HeaderMap,
) -> Response {
    // Redirect authenticated users to the app (check access token, then refresh token)
    if let Some(token) = get_cookie(&headers, ACCESS_COOKIE_NAME) {
        if state.jwt.validate_access_token(token).is_ok() {
            return Redirect::temporary(state.app_path).into_response();
        }
    }
    if let Some(token) = get_cookie(&headers, REFRESH_COOKIE_NAME) {
        if state.jwt.validate_refresh_token(token).is_ok() {
            return Redirect::temporary(state.app_path).into_response();
        }
    }
    (
        [
            (header::CONTENT_TYPE, "text/html"),
            (header::CACHE_CONTROL, NO_CACHE),
        ],
        state.login_index_html,
    )
        .into_response()
}

/// Serve login assets with base path rewriting
pub async fn login_handler(
    axum::extract::State(state): axum::extract::State<Arc<AssetsState>>,
    path: Option<axum::extract::Path<String>>,
) -> Response {
    let path = normalize_path(path.as_ref());
    if path.ends_with(".html") {
        if let Some(&processed) = state.processed_login_html.get(path) {
            return (
                [
                    (header::CONTENT_TYPE, "text/html"),
                    (header::CACHE_CONTROL, NO_CACHE),
                ],
                processed,
            )
                .into_response();
        }
    }
    serve_asset::<LoginAssets>(path)
}

/// Serve app assets directly (no base path rewriting) - requires auth
pub async fn app_handler_direct(
    auth: Result<RequireAuth, AuthError>,
    path: Option<axum::extract::Path<String>>,
) -> Response {
    if let Err(e) = auth {
        return e.into_response();
    }
    serve_asset::<AppAssets>(normalize_path(path.as_ref()))
}

/// Serve app assets with base path rewriting - requires auth
pub async fn app_handler(
    axum::extract::State(state): axum::extract::State<Arc<AssetsState>>,
    auth: Result<RequireAuth, AuthError>,
    path: Option<axum::extract::Path<String>>,
) -> Response {
    if let Err(e) = auth {
        return e.into_response();
    }

    let path = normalize_path(path.as_ref());
    if path.ends_with(".html") {
        if let Some(&processed) = state.processed_app_html.get(path) {
            return (
                [
                    (header::CONTENT_TYPE, "text/html"),
                    (header::CACHE_CONTROL, NO_CACHE),
                ],
                processed,
            )
                .into_response();
        }
    }
    serve_asset::<AppAssets>(path)
}
