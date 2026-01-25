//! Axum extractors for authentication.

use std::cell::RefCell;

use axum::{extract::FromRequestParts, http::request::Parts};

use super::cookie::{ACCESS_COOKIE_NAME, REFRESH_COOKIE_NAME, get_cookie};
use super::errors::{ApiAuthError, AssetAuthError, AuthErrorKind};
use super::ip::extract_client_ip;
use super::state::{HasAssetAuthState, HasAuthState};
use super::types::{ActivatedAuthenticatedUser, AuthenticatedUser};

tokio::task_local! {
    /// Task-local storage for the new access token cookie.
    /// Used to pass the cookie from the auth extractor to the response middleware.
    pub static NEW_ACCESS_TOKEN_COOKIE: RefCell<Option<String>>;
}

/// Extension type to pass new access token cookie to response layer.
#[derive(Clone)]
pub struct NewAccessTokenCookie(pub String);

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
    let client_ip = extract_client_ip(parts, state.ip_extractor())
        .map_err(|_| AuthErrorKind::NotAuthenticated)?;

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

// =============================================================================
// API Extractors
// =============================================================================

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
        authenticate_request(parts, state)
            .await
            .map(ApiAuth)
            .map_err(ApiAuthError::from)
    }
}

/// Extractor for API endpoints that require an activated account.
/// Same as `ApiAuth` but also verifies the user has activated their account.
pub struct ActivatedApiAuth(pub ActivatedAuthenticatedUser);

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
        Ok(ActivatedApiAuth(ActivatedAuthenticatedUser {
            claims: user.claims,
            user_id: id,
        }))
    }
}

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
// Asset Extractors
// =============================================================================

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
