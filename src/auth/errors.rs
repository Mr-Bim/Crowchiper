//! Authentication error types.

use axum::{
    http::header,
    response::{IntoResponse, Response},
};

use super::cookie::{ACCESS_COOKIE_NAME, REFRESH_COOKIE_NAME};
use crate::server_config;

/// Internal auth error kind used by the core authentication logic.
#[derive(Debug)]
pub enum AuthErrorKind {
    NotAuthenticated,
    InvalidToken,
    TokenRevoked,
    UserNotFound,
    AccountNotActivated,
    InsufficientRole,
    DatabaseError,
}

/// API authentication errors (returns JSON and clears cookies).
#[derive(Debug)]
pub struct ApiAuthError {
    pub(super) kind: AuthErrorKind,
}

impl ApiAuthError {
    pub(super) fn new(kind: AuthErrorKind) -> Self {
        Self { kind }
    }

    fn status_code(&self) -> axum::http::StatusCode {
        use axum::http::StatusCode;
        match self.kind {
            AuthErrorKind::NotAuthenticated
            | AuthErrorKind::InvalidToken
            | AuthErrorKind::TokenRevoked
            | AuthErrorKind::UserNotFound => StatusCode::UNAUTHORIZED,
            AuthErrorKind::AccountNotActivated | AuthErrorKind::InsufficientRole => {
                StatusCode::FORBIDDEN
            }
            AuthErrorKind::DatabaseError => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    fn message(&self) -> &'static str {
        match self.kind {
            AuthErrorKind::NotAuthenticated => "Not authenticated",
            AuthErrorKind::InvalidToken => "Invalid or expired token",
            AuthErrorKind::TokenRevoked => "Token has been revoked",
            AuthErrorKind::UserNotFound => "User not found",
            AuthErrorKind::AccountNotActivated => "Account not activated",
            AuthErrorKind::InsufficientRole => "Insufficient permissions",
            AuthErrorKind::DatabaseError => "Database error",
        }
    }
}

impl IntoResponse for ApiAuthError {
    fn into_response(self) -> Response {
        use axum::Json;
        use axum::http::HeaderValue;
        use serde::Serialize;

        #[derive(Serialize)]
        struct ErrorResponse {
            error: &'static str,
        }

        // Clear both cookies on auth errors
        let secure = if server_config::secure_cookies() {
            "; Secure"
        } else {
            ""
        };
        let clear_access = format!(
            "{}=; HttpOnly; SameSite=Strict; Path=/; Max-Age=0{}",
            ACCESS_COOKIE_NAME, secure
        );
        let clear_refresh = format!(
            "{}=; HttpOnly; SameSite=Strict; Path=/; Max-Age=0{}",
            REFRESH_COOKIE_NAME, secure
        );

        let mut response = (
            self.status_code(),
            Json(ErrorResponse {
                error: self.message(),
            }),
        )
            .into_response();

        let headers = response.headers_mut();
        if let Ok(value) = HeaderValue::from_str(&clear_access) {
            headers.append(header::SET_COOKIE, value);
        }
        if let Ok(value) = HeaderValue::from_str(&clear_refresh) {
            headers.append(header::SET_COOKIE, value);
        }

        response
    }
}

/// Asset authentication error - redirects to login without clearing cookies.
/// Used for protected asset routes where we want to preserve refresh tokens.
#[derive(Debug)]
pub struct AssetAuthError {
    pub login_path: String,
}

impl IntoResponse for AssetAuthError {
    fn into_response(self) -> Response {
        axum::response::Redirect::temporary(&self.login_path).into_response()
    }
}
