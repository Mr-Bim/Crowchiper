//! JWT authentication for API routes.
//!
//! Uses a dual-token system:
//! - Access tokens: Short-lived (5 min), stateless, no database check
//! - Refresh tokens: Long-lived (2 weeks), tracked in database with JTI
//!
//! When an access token expires, the middleware automatically issues a new one
//! if the refresh token is still valid (not expired, not revoked).

use axum::{
    extract::{ConnectInfo, FromRequestParts},
    http::{header, request::Parts},
    response::{IntoResponse, Response},
};
use std::net::SocketAddr;

use crate::db::Database;
use crate::jwt::{AccessClaims, JwtConfig};

/// Cookie name for the access token (short-lived, 5 minutes).
pub const ACCESS_COOKIE_NAME: &str = "access_token";

/// Cookie name for the refresh token (long-lived, 2 weeks).
pub const REFRESH_COOKIE_NAME: &str = "refresh_token";

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

/// Authenticated user information extracted from JWT.
#[derive(Debug, Clone)]
pub struct AuthenticatedUser {
    /// JWT claims from the access token
    pub claims: AccessClaims,
    /// Database user ID
    pub user_id: i64,
    /// Client IP address (if available)
    pub client_ip: Option<String>,
}

/// Result of authentication that may include a new access token cookie.
pub struct AuthResult {
    pub user: AuthenticatedUser,
    /// New access token cookie to set (if token was refreshed)
    pub new_access_cookie: Option<String>,
}

/// API authentication errors (returns JSON instead of redirects).
#[derive(Debug)]
pub enum ApiAuthError {
    NotAuthenticated,
    InvalidToken,
    TokenRevoked,
    UserNotFound,
    AccountNotActivated,
    DatabaseError,
}

impl ApiAuthError {
    fn status_code(&self) -> axum::http::StatusCode {
        use axum::http::StatusCode;
        match self {
            Self::NotAuthenticated | Self::InvalidToken | Self::TokenRevoked => {
                StatusCode::UNAUTHORIZED
            }
            Self::UserNotFound => StatusCode::UNAUTHORIZED,
            Self::AccountNotActivated => StatusCode::FORBIDDEN,
            Self::DatabaseError => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    fn message(&self) -> &'static str {
        match self {
            Self::NotAuthenticated => "Not authenticated",
            Self::InvalidToken => "Invalid or expired token",
            Self::TokenRevoked => "Token has been revoked",
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
    fn secure_cookies(&self) -> bool;
}

/// Extractor for API endpoints that require authentication.
/// Validates the access token (short-lived, stateless).
/// If expired, attempts to refresh using the refresh token.
/// Returns JSON errors instead of redirects.
pub struct ApiAuth(pub AuthenticatedUser);

impl<S> FromRequestParts<S> for ApiAuth
where
    S: HasAuthState + Send + Sync,
{
    type Rejection = ApiAuthError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let client_ip = extract_client_ip(&parts.headers, parts);

        // Try to validate access token first
        if let Some(access_token) = get_cookie(&parts.headers, ACCESS_COOKIE_NAME) {
            if let Ok(claims) = state.jwt().validate_access_token(access_token) {
                // Access token is valid - look up user
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

                return Ok(ApiAuth(AuthenticatedUser {
                    claims,
                    user_id: user.id,
                    client_ip,
                }));
            }
        }

        // Access token missing or invalid - try refresh token
        let refresh_token = get_cookie(&parts.headers, REFRESH_COOKIE_NAME)
            .ok_or(ApiAuthError::NotAuthenticated)?;

        // Validate refresh token
        let refresh_claims = state
            .jwt()
            .validate_refresh_token(refresh_token)
            .map_err(|_| ApiAuthError::InvalidToken)?;

        // Check if refresh token is in database (not revoked)
        let active_token = state
            .db()
            .tokens()
            .get_by_jti(&refresh_claims.jti)
            .await
            .map_err(|e| {
                tracing::error!("Failed to check token: {}", e);
                ApiAuthError::DatabaseError
            })?
            .ok_or(ApiAuthError::TokenRevoked)?;

        // Look up user
        let user = state
            .db()
            .users()
            .get_by_uuid(&refresh_claims.sub)
            .await
            .map_err(|e| {
                tracing::error!("Failed to get user: {}", e);
                ApiAuthError::DatabaseError
            })?
            .ok_or(ApiAuthError::UserNotFound)?;

        if !user.activated {
            return Err(ApiAuthError::AccountNotActivated);
        }

        // Update IP if changed
        if let Some(ref ip) = client_ip {
            let ip_changed = active_token.last_ip.as_ref() != Some(ip);
            if ip_changed {
                if let Err(e) = state.db().tokens().update_ip(&refresh_claims.jti, ip).await {
                    tracing::warn!("Failed to update token IP: {}", e);
                }
            }
        }

        // Generate new access token
        let access_result = state
            .jwt()
            .generate_access_token(&user.uuid, &user.username, user.role)
            .map_err(|e| {
                tracing::error!("Failed to generate access token: {}", e);
                ApiAuthError::DatabaseError
            })?;

        // Store the new access token cookie in extensions for the response layer
        let secure = if state.secure_cookies() {
            "; Secure"
        } else {
            ""
        };
        let new_cookie = format!(
            "{}={}; HttpOnly; SameSite=Strict; Path=/; Max-Age={}{}",
            ACCESS_COOKIE_NAME, access_result.token, access_result.duration, secure
        );
        parts.extensions.insert(NewAccessTokenCookie(new_cookie));

        // Parse the new access token to get claims
        let claims = state
            .jwt()
            .validate_access_token(&access_result.token)
            .map_err(|_| ApiAuthError::DatabaseError)?;

        Ok(ApiAuth(AuthenticatedUser {
            claims,
            user_id: user.id,
            client_ip,
        }))
    }
}

/// Extension type to pass new access token cookie to response layer.
#[derive(Clone)]
pub struct NewAccessTokenCookie(pub String);

/// Extract client IP address from headers or connection info.
/// Checks X-Forwarded-For first (for reverse proxy), then falls back to extensions.
fn extract_client_ip(headers: &axum::http::HeaderMap, parts: &Parts) -> Option<String> {
    // Check X-Forwarded-For header first (reverse proxy)
    if let Some(forwarded_for) = headers.get("x-forwarded-for") {
        if let Ok(value) = forwarded_for.to_str() {
            // X-Forwarded-For can contain multiple IPs, take the first (original client)
            if let Some(first_ip) = value.split(',').next() {
                let ip = first_ip.trim();
                if !ip.is_empty() {
                    return Some(ip.to_string());
                }
            }
        }
    }

    // Try to get from ConnectInfo extension
    parts
        .extensions
        .get::<ConnectInfo<SocketAddr>>()
        .map(|ci| ci.0.ip().to_string())
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
            HeaderValue::from_static("access_token=abc123"),
        );

        assert_eq!(get_cookie(&headers, "access_token"), Some("abc123"));
    }

    #[test]
    fn test_get_cookie_multiple() {
        let mut headers = axum::http::HeaderMap::new();
        headers.insert(
            header::COOKIE,
            HeaderValue::from_static("foo=bar; access_token=abc123; refresh_token=xyz789"),
        );

        assert_eq!(get_cookie(&headers, "access_token"), Some("abc123"));
        assert_eq!(get_cookie(&headers, "refresh_token"), Some("xyz789"));
        assert_eq!(get_cookie(&headers, "foo"), Some("bar"));
    }

    #[test]
    fn test_get_cookie_not_found() {
        let mut headers = axum::http::HeaderMap::new();
        headers.insert(header::COOKIE, HeaderValue::from_static("foo=bar"));

        assert_eq!(get_cookie(&headers, "access_token"), None);
    }

    #[test]
    fn test_get_cookie_no_header() {
        let headers = axum::http::HeaderMap::new();
        assert_eq!(get_cookie(&headers, "access_token"), None);
    }

    #[test]
    fn test_get_cookie_with_spaces() {
        let mut headers = axum::http::HeaderMap::new();
        headers.insert(
            header::COOKIE,
            HeaderValue::from_static("  access_token = abc123  ; foo=bar"),
        );

        assert_eq!(get_cookie(&headers, "access_token"), Some("abc123"));
    }
}
