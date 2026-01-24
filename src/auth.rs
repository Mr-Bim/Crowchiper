//! JWT authentication for API routes.
//!
//! Uses a dual-token system:
//! - Access tokens: Short-lived (5 min), stateless, no database check
//! - Refresh tokens: Long-lived (2 weeks), tracked in database with JTI
//!
//! When an access token expires, the middleware automatically issues a new one
//! if the refresh token is still valid (not expired, not revoked).

use std::cell::RefCell;
use std::net::SocketAddr;

use axum::{
    extract::{ConnectInfo, FromRequestParts},
    http::{header, request::Parts},
    response::{IntoResponse, Response},
};

tokio::task_local! {
    /// Task-local storage for the new access token cookie.
    /// Used to pass the cookie from the auth extractor to the response middleware.
    pub static NEW_ACCESS_TOKEN_COOKIE: RefCell<Option<String>>;
}

use crate::cli::ClientIpHeader;
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

/// Extract client IP address based on configuration.
///
/// If `ip_header` is set, extracts IP from the specified header and returns an error
/// if the header is missing or invalid (does NOT fall back to SocketAddr).
///
/// If `ip_header` is None, uses the SocketAddr from ConnectInfo.
pub fn extract_client_ip(
    parts: &Parts,
    ip_header: Option<&ClientIpHeader>,
) -> Result<String, &'static str> {
    match ip_header {
        Some(header) => extract_ip_from_header(parts, header),
        None => parts
            .extensions
            .get::<ConnectInfo<SocketAddr>>()
            .map(|ci| ci.0.ip().to_string())
            .ok_or("No client IP available"),
    }
}

/// Extract IP from a specific header. Returns error if header is missing or invalid.
fn extract_ip_from_header(parts: &Parts, header: &ClientIpHeader) -> Result<String, &'static str> {
    let header_name = match header {
        ClientIpHeader::CFConnectingIP => "cf-connecting-ip",
        ClientIpHeader::XRealIp => "x-real-ip",
        ClientIpHeader::XForwardFor => "x-forwarded-for",
        ClientIpHeader::Forward => "forwarded",
    };

    let header_value = parts
        .headers
        .get(header_name)
        .ok_or("IP header not present")?
        .to_str()
        .map_err(|_| "IP header contains invalid characters")?;

    match header {
        ClientIpHeader::CFConnectingIP | ClientIpHeader::XRealIp => {
            // Single IP value
            let ip = header_value.trim();
            if ip.is_empty() {
                return Err("IP header is empty");
            }
            Ok(ip.to_string())
        }
        ClientIpHeader::XForwardFor => {
            // Comma-separated list, take the first (original client)
            let ip = header_value
                .split(',')
                .next()
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .ok_or("X-Forwarded-For header has no valid IP")?;
            Ok(ip.to_string())
        }
        ClientIpHeader::Forward => {
            // RFC 7239 format: "for=192.0.2.60;proto=http;by=203.0.113.43"
            // Can have multiple comma-separated entries, take first "for=" value
            for part in header_value.split(',') {
                for param in part.split(';') {
                    let param = param.trim();
                    if let Some(value) = param.strip_prefix("for=") {
                        let ip = value.trim().trim_matches('"');
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
                            return Ok(ip.to_string());
                        }
                    }
                }
            }
            Err("Forwarded header has no valid 'for' parameter")
        }
    }
}

/// Authenticated user information extracted from JWT.
#[derive(Debug, Clone)]
pub struct AuthenticatedUser {
    /// JWT claims from the access token
    pub claims: AccessClaims,
    /// Database user ID
    pub user_id: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct ActicatedAuthenticatedUser {
    /// JWT claims from the access token
    pub claims: AccessClaims,
    /// Database user ID
    pub user_id: i64,
}

/// Internal auth error type used by the core authentication logic.
#[derive(Debug)]
pub enum AuthErrorKind {
    NotAuthenticated,
    InvalidToken,
    TokenRevoked,
    UserNotFound,
    AccountNotActivated,
    DatabaseError,
}

/// Core authentication logic shared between API and Asset auth extractors.
/// Returns the authenticated user or an error kind.
async fn authenticate_request<S>(
    parts: &Parts,
    state: &S,
) -> Result<AuthenticatedUser, AuthErrorKind>
where
    S: HasAuthState + Send + Sync,
{
    // Extract client IP
    let client_ip =
        extract_client_ip(parts, state.ip_header()).map_err(|_| AuthErrorKind::NotAuthenticated)?;

    // Try to validate access token first
    if let Some(access_token) = get_cookie(&parts.headers, ACCESS_COOKIE_NAME) {
        if let Ok(claims) = state.jwt().validate_access_token(access_token) {
            if claims.ipaddr == client_ip {
                return Ok(AuthenticatedUser {
                    claims,
                    user_id: None,
                });
            }
        }
    }

    // Access token missing or invalid - try refresh token
    let refresh_token =
        get_cookie(&parts.headers, REFRESH_COOKIE_NAME).ok_or(AuthErrorKind::NotAuthenticated)?;

    // Validate refresh token
    let refresh_claims = state
        .jwt()
        .validate_refresh_token(refresh_token)
        .map_err(|_| AuthErrorKind::InvalidToken)?;

    // Check if refresh token is in database (not revoked)
    let active_token = state
        .db()
        .tokens()
        .get_by_jti(&refresh_claims.jti)
        .await
        .map_err(|e| {
            tracing::error!("Failed to check token: {}", e);
            AuthErrorKind::DatabaseError
        })?
        .ok_or(AuthErrorKind::TokenRevoked)?;

    // Look up user
    let user = state
        .db()
        .users()
        .get_by_uuid(&refresh_claims.sub)
        .await
        .map_err(|e| {
            tracing::error!("Failed to get user: {}", e);
            AuthErrorKind::DatabaseError
        })?
        .ok_or(AuthErrorKind::UserNotFound)?;

    if !user.activated {
        return Err(AuthErrorKind::AccountNotActivated);
    }

    // Update IP if changed
    let ip_changed = active_token.last_ip.as_ref() != Some(&client_ip);
    if ip_changed {
        if let Err(e) = state
            .db()
            .tokens()
            .update_ip(&refresh_claims.jti, &client_ip)
            .await
        {
            tracing::warn!("Failed to update token IP: {}", e);
        }
    }

    // Generate new access token
    let access_result = state
        .jwt()
        .generate_access_token(&user.uuid, &user.username, user.role, &client_ip)
        .map_err(|e| {
            tracing::error!("Failed to generate access token: {}", e);
            AuthErrorKind::DatabaseError
        })?;

    // Store the new access token cookie in task-local for the response middleware
    let secure = if state.secure_cookies() {
        "; Secure"
    } else {
        ""
    };
    let new_cookie = format!(
        "{}={}; HttpOnly; SameSite=Strict; Path=/; Max-Age={}{}",
        ACCESS_COOKIE_NAME, access_result.token, access_result.duration, secure
    );
    let _ = NEW_ACCESS_TOKEN_COOKIE.try_with(|cell| {
        cell.borrow_mut().replace(new_cookie);
    });

    // Parse the new access token to get claims
    let claims = state
        .jwt()
        .validate_access_token(&access_result.token)
        .map_err(|_| AuthErrorKind::DatabaseError)?;

    Ok(AuthenticatedUser {
        claims,
        user_id: Some(user.id),
    })
}

/// API authentication errors (returns JSON and clears cookies).
#[derive(Debug)]
pub struct ApiAuthError(AuthErrorKind);

impl From<AuthErrorKind> for ApiAuthError {
    fn from(kind: AuthErrorKind) -> Self {
        Self(kind)
    }
}

impl ApiAuthError {
    fn status_code(&self) -> axum::http::StatusCode {
        use axum::http::StatusCode;
        match self.0 {
            AuthErrorKind::NotAuthenticated
            | AuthErrorKind::InvalidToken
            | AuthErrorKind::TokenRevoked
            | AuthErrorKind::UserNotFound => StatusCode::UNAUTHORIZED,
            AuthErrorKind::AccountNotActivated => StatusCode::FORBIDDEN,
            AuthErrorKind::DatabaseError => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    fn message(&self) -> &'static str {
        match self.0 {
            AuthErrorKind::NotAuthenticated => "Not authenticated",
            AuthErrorKind::InvalidToken => "Invalid or expired token",
            AuthErrorKind::TokenRevoked => "Token has been revoked",
            AuthErrorKind::UserNotFound => "User not found",
            AuthErrorKind::AccountNotActivated => "Account not activated",
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
        let clear_access = format!(
            "{}=; HttpOnly; SameSite=Strict; Path=/; Max-Age=0",
            ACCESS_COOKIE_NAME
        );
        let clear_refresh = format!(
            "{}=; HttpOnly; SameSite=Strict; Path=/; Max-Age=0",
            REFRESH_COOKIE_NAME
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

/// Trait for state types that support API authentication.
pub trait HasAuthState {
    fn jwt(&self) -> &JwtConfig;
    fn db(&self) -> &Database;
    fn secure_cookies(&self) -> bool;
    fn ip_header(&self) -> Option<&ClientIpHeader>;
}

/// Extractor for API endpoints that require authentication.
/// Validates the access token (short-lived, stateless).
/// If expired, attempts to refresh using the refresh token.
/// Returns JSON errors instead of redirects.
pub struct ApiAuth(pub AuthenticatedUser);
pub struct ActivatedApiAuth(pub ActicatedAuthenticatedUser);

impl<S> FromRequestParts<S> for ActivatedApiAuth
where
    S: HasAuthState + Send + Sync,
{
    type Rejection = ApiAuthError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let ApiAuth(user) = ApiAuth::from_request_parts(parts, state).await?;

        let id = match user.user_id {
            Some(id) => id,
            None => {
                let db_user = state
                    .db()
                    .users()
                    .get_by_uuid(&user.claims.sub)
                    .await
                    .map_err(|_| ApiAuthError(AuthErrorKind::DatabaseError))?
                    .ok_or(ApiAuthError(AuthErrorKind::UserNotFound))?;

                if !db_user.activated {
                    return Err(ApiAuthError(AuthErrorKind::AccountNotActivated));
                }
                db_user.id
            }
        };
        Ok(ActivatedApiAuth(ActicatedAuthenticatedUser {
            claims: user.claims,
            user_id: id,
        }))
    }
}

impl<S> FromRequestParts<S> for ApiAuth
where
    S: HasAuthState + Send + Sync,
{
    type Rejection = ApiAuthError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        authenticate_request(parts, state)
            .await
            .map(ApiAuth)
            .map_err(ApiAuthError::from)
    }
}

/// Extension type to pass new access token cookie to response layer.
#[derive(Clone)]
pub struct NewAccessTokenCookie(pub String);

/// Optional authentication extractor - never fails, returns Option<AuthenticatedUser>.
/// Useful for endpoints that work both authenticated and unauthenticated.
pub struct MaybeAuth(pub Option<AuthenticatedUser>);

impl<S> FromRequestParts<S> for MaybeAuth
where
    S: HasAuthState + Send + Sync,
{
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        Ok(MaybeAuth(authenticate_request(parts, state).await.ok()))
    }
}

// =============================================================================
// Asset Authentication (redirects to login, doesn't clear cookies)
// =============================================================================

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

/// Trait for state types that support asset authentication.
pub trait HasAssetAuthState: HasAuthState {
    fn login_path(&self) -> &str;
}

/// Extractor for asset endpoints that require authentication.
/// On failure, redirects to login WITHOUT clearing cookies.
/// This allows the refresh token to work on subsequent API calls.
pub struct AssetAuth(pub AuthenticatedUser);

impl<S> FromRequestParts<S> for AssetAuth
where
    S: HasAssetAuthState + Send + Sync,
{
    type Rejection = AssetAuthError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        authenticate_request(parts, state)
            .await
            .map(AssetAuth)
            .map_err(|_| AssetAuthError {
                login_path: state.login_path().to_string(),
            })
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
