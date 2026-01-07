//! JWT authentication for API routes.

use axum::{
    extract::FromRequestParts,
    http::{header, request::Parts},
    response::{IntoResponse, Response},
};

use crate::db::Database;
use crate::jwt::{Claims, JwtConfig};

/// Cookie name for the authentication token.
pub const AUTH_COOKIE_NAME: &str = "auth_token";

/// Extract a cookie value from the Cookie header.
pub fn get_cookie<'a>(headers: &'a axum::http::HeaderMap, name: &str) -> Option<&'a str> {
    let cookie_header = headers.get(header::COOKIE)?.to_str().ok()?;
    for part in cookie_header.split(';') {
        let part = part.trim();
        if let Some((key, value)) = part.split_once('=') {
            if key.trim() == name {
                return Some(value.trim());
            }
        }
    }
    None
}

/// Authenticated user information extracted from JWT and database.
#[derive(Debug, Clone)]
pub struct AuthenticatedUser {
    /// JWT claims
    pub claims: Claims,
    /// Database user ID
    pub user_id: i64,
}

/// API authentication errors (returns JSON instead of redirects).
#[derive(Debug)]
pub enum ApiAuthError {
    NotAuthenticated,
    InvalidToken,
    UserNotFound,
    AccountNotActivated,
    DatabaseError,
}

impl ApiAuthError {
    fn status_code(&self) -> axum::http::StatusCode {
        use axum::http::StatusCode;
        match self {
            Self::NotAuthenticated | Self::InvalidToken => StatusCode::UNAUTHORIZED,
            Self::UserNotFound => StatusCode::UNAUTHORIZED,
            Self::AccountNotActivated => StatusCode::FORBIDDEN,
            Self::DatabaseError => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    fn message(&self) -> &'static str {
        match self {
            Self::NotAuthenticated => "Not authenticated",
            Self::InvalidToken => "Invalid or expired token",
            Self::UserNotFound => "User not found",
            Self::AccountNotActivated => "Account not activated",
            Self::DatabaseError => "Database error",
        }
    }
}

impl IntoResponse for ApiAuthError {
    fn into_response(self) -> Response {
        use axum::Json;
        use serde::Serialize;

        #[derive(Serialize)]
        struct ErrorResponse {
            error: &'static str,
        }

        (
            self.status_code(),
            Json(ErrorResponse {
                error: self.message(),
            }),
        )
            .into_response()
    }
}

/// Trait for state types that support API authentication.
pub trait HasAuthState {
    fn jwt(&self) -> &JwtConfig;
    fn db(&self) -> &Database;
}

/// Extractor for API endpoints that require authentication.
/// Returns JSON errors instead of redirects.
pub struct ApiAuth(pub AuthenticatedUser);

impl<S> FromRequestParts<S> for ApiAuth
where
    S: HasAuthState + Send + Sync,
{
    type Rejection = ApiAuthError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        // Get auth token from cookie
        let token =
            get_cookie(&parts.headers, AUTH_COOKIE_NAME).ok_or(ApiAuthError::NotAuthenticated)?;

        // Validate token
        let claims = state
            .jwt()
            .validate_token(token)
            .map_err(|_| ApiAuthError::InvalidToken)?;

        // Look up user in database
        let user = state
            .db()
            .users()
            .get_by_uuid(&claims.sub)
            .await
            .map_err(|e| {
                tracing::error!("Failed to get user: {}", e);
                ApiAuthError::DatabaseError
            })?
            .ok_or(ApiAuthError::UserNotFound)?;

        if !user.activated {
            return Err(ApiAuthError::AccountNotActivated);
        }

        Ok(ApiAuth(AuthenticatedUser {
            claims,
            user_id: user.id,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    #[test]
    fn test_get_cookie_simple() {
        let mut headers = axum::http::HeaderMap::new();
        headers.insert(
            header::COOKIE,
            HeaderValue::from_static("auth_token=abc123"),
        );

        assert_eq!(get_cookie(&headers, "auth_token"), Some("abc123"));
    }

    #[test]
    fn test_get_cookie_multiple() {
        let mut headers = axum::http::HeaderMap::new();
        headers.insert(
            header::COOKIE,
            HeaderValue::from_static("foo=bar; auth_token=abc123; baz=qux"),
        );

        assert_eq!(get_cookie(&headers, "auth_token"), Some("abc123"));
        assert_eq!(get_cookie(&headers, "foo"), Some("bar"));
        assert_eq!(get_cookie(&headers, "baz"), Some("qux"));
    }

    #[test]
    fn test_get_cookie_not_found() {
        let mut headers = axum::http::HeaderMap::new();
        headers.insert(header::COOKIE, HeaderValue::from_static("foo=bar"));

        assert_eq!(get_cookie(&headers, "auth_token"), None);
    }

    #[test]
    fn test_get_cookie_no_header() {
        let headers = axum::http::HeaderMap::new();
        assert_eq!(get_cookie(&headers, "auth_token"), None);
    }

    #[test]
    fn test_get_cookie_with_spaces() {
        let mut headers = axum::http::HeaderMap::new();
        headers.insert(
            header::COOKIE,
            HeaderValue::from_static("  auth_token = abc123  ; foo=bar"),
        );

        assert_eq!(get_cookie(&headers, "auth_token"), Some("abc123"));
    }
}
