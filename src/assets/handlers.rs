use axum::{
    extract::{Path, State},
    http::HeaderMap,
    response::{IntoResponse, Redirect, Response},
};

use crate::auth::{
    ACCESS_COOKIE_NAME, AdminOnly, AnyRole, ProtectedAsset, REFRESH_COOKIE_NAME, get_cookie,
};

use super::config::AssetsState;
use super::response::normalize_path;

/// Serve login assets (public, no auth required)
pub async fn login_handler(
    State(state): State<AssetsState>,
    path: Option<Path<String>>,
) -> Response {
    let path = normalize_path(path.as_ref());
    (state.login.server)(path, &state.login, state.html_responder)
}

/// Serve login index page, redirecting authenticated users to the app.
pub async fn login_index_handler(State(state): State<AssetsState>, headers: HeaderMap) -> Response {
    // Redirect authenticated users to the app (check access token, then refresh token)
    if let Some(token) = get_cookie(&headers, ACCESS_COOKIE_NAME) {
        if state.jwt.validate_access_token(token).is_ok() {
            return Redirect::temporary(state.app.path).into_response();
        }
    }
    // For refresh tokens, also check database to ensure token wasn't revoked
    if let Some(token) = get_cookie(&headers, REFRESH_COOKIE_NAME) {
        if let Ok(claims) = state.jwt.validate_refresh_token(token) {
            // Check if token is still in database (not revoked)
            if state
                .db
                .tokens()
                .get_by_jti(&claims.jti)
                .await
                .ok()
                .flatten()
                .is_some()
            {
                return Redirect::temporary(state.app.path).into_response();
            }
        }
    }
    (state.html_responder)(state.login_index_html, state.login.csp_header)
}

/// Serve app assets (protected, JWT required)
pub async fn app_handler(
    State(state): State<AssetsState>,
    _: ProtectedAsset<AnyRole>,
    path: Option<Path<String>>,
) -> Response {
    let path = normalize_path(path.as_ref());
    (state.app.server)(path, &state.app, state.html_responder)
}

/// Serve dashboard assets (protected, JWT required, admin only)
pub async fn dashboard_handler(
    State(state): State<AssetsState>,
    _: ProtectedAsset<AdminOnly>,
    path: Option<Path<String>>,
) -> Response {
    let path = normalize_path(path.as_ref());
    (state.dashboard.server)(path, &state.dashboard, state.html_responder)
}
