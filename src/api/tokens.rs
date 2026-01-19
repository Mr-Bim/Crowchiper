//! Token management API endpoints.
//!
//! - POST `/refresh` - Exchange refresh token for new access token
//! - POST `/logout` - Revoke refresh token and clear cookies
//! - GET `/` - List active refresh tokens for current user
//! - DELETE `/{jti}` - Revoke specific refresh token (own token or admin)

use axum::{
    Json, Router,
    extract::{Path, State},
    http::{StatusCode, header::SET_COOKIE},
    response::IntoResponse,
    routing::{delete, get, post},
};
use serde::Serialize;
use std::sync::Arc;

use super::error::{ApiError, ResultExt};
use crate::auth::{
    ACCESS_COOKIE_NAME, ActivatedApiAuth, ApiAuth, HasAuthState, REFRESH_COOKIE_NAME, get_cookie,
};
use crate::db::{Database, UserRole};
use crate::jwt::JwtConfig;

#[derive(Clone)]
pub struct TokensState {
    pub db: Database,
    pub jwt: Arc<JwtConfig>,
    pub secure_cookies: bool,
}

impl HasAuthState for TokensState {
    fn jwt(&self) -> &JwtConfig {
        &self.jwt
    }

    fn db(&self) -> &Database {
        &self.db
    }

    fn secure_cookies(&self) -> bool {
        self.secure_cookies
    }
}

pub fn router(state: TokensState) -> Router {
    Router::new()
        .route("/", get(list_tokens))
        .route("/verify", get(verify_token))
        .route("/logout", post(logout))
        .route("/{jti}", delete(revoke_token))
        .with_state(state)
}

#[derive(Serialize)]
struct TokenInfo {
    jti: String,
    last_ip: Option<String>,
    issued_at: String,
    expires_at: String,
    is_current: bool,
}

#[derive(Serialize)]
struct ListTokensResponse {
    tokens: Vec<TokenInfo>,
}

/// Verify that the current access token is still valid.
/// Returns 200 if valid, 401 if not.
/// This is a lightweight endpoint for checking auth status (e.g., on bfcache restore).
async fn verify_token(ApiAuth(_auth): ApiAuth) -> impl IntoResponse {
    StatusCode::OK
}

/// List all active refresh tokens for the current user.
async fn list_tokens(
    State(state): State<TokensState>,
    ActivatedApiAuth(auth): ActivatedApiAuth,
) -> Result<impl IntoResponse, ApiError> {
    let tokens = state
        .db
        .tokens()
        .list_by_user(auth.user_id)
        .await
        .db_err("Failed to list tokens")?;

    // Get current refresh token JTI from cookie to mark as current
    // Note: We don't have access to the refresh token here since ApiAuth uses access token
    // So we'll mark based on the access token's JTI (which won't match refresh tokens)
    // This is intentional - refresh tokens are a separate concern
    let token_infos: Vec<TokenInfo> = tokens
        .into_iter()
        .map(|t| TokenInfo {
            is_current: false, // Will be updated by frontend based on context
            jti: t.jti,
            last_ip: t.last_ip,
            issued_at: t.issued_at,
            expires_at: t.expires_at,
        })
        .collect();

    Ok((
        StatusCode::OK,
        Json(ListTokensResponse {
            tokens: token_infos,
        }),
    ))
}

/// Logout - revoke refresh token and clear both cookies.
async fn logout(
    State(state): State<TokensState>,
    request: axum::extract::Request,
) -> Result<impl IntoResponse, ApiError> {
    let (parts, _body) = request.into_parts();

    // Try to get and revoke the refresh token
    if let Some(refresh_token) = get_cookie(&parts.headers, REFRESH_COOKIE_NAME) {
        if let Ok(claims) = state.jwt.validate_refresh_token(refresh_token) {
            // Delete the refresh token from database
            let _ = state.db.tokens().delete_by_jti(&claims.jti).await;
        }
    }

    // Clear both cookies using AppendHeaders to send multiple Set-Cookie headers
    use axum::response::AppendHeaders;
    let secure = if state.secure_cookies { "; Secure" } else { "" };
    let clear_access = format!(
        "{}=; HttpOnly; SameSite=Strict; Path=/; Max-Age=0{}",
        ACCESS_COOKIE_NAME, secure
    );
    let clear_refresh = format!(
        "{}=; HttpOnly; SameSite=Strict; Path=/; Max-Age=0{}",
        REFRESH_COOKIE_NAME, secure
    );

    Ok((
        StatusCode::OK,
        AppendHeaders([(SET_COOKIE, clear_access), (SET_COOKIE, clear_refresh)]),
        Json(serde_json::json!({ "success": true })),
    ))
}

#[derive(Serialize)]
struct RevokeResponse {
    revoked: bool,
}

/// Revoke a specific refresh token by JTI.
/// Users can revoke their own tokens, admins can revoke any token.
async fn revoke_token(
    State(state): State<TokensState>,
    ActivatedApiAuth(auth): ActivatedApiAuth,
    Path(jti): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    // Check if the token belongs to the current user or if user is admin
    let token = state
        .db
        .tokens()
        .get_by_jti(&jti)
        .await
        .db_err("Failed to get token")?;

    if let Some(token) = token {
        // Check authorization: must be own token or admin
        if token.user_id != auth.user_id && auth.claims.role != UserRole::Admin {
            return Err(ApiError::forbidden("Cannot revoke another user's token"));
        }

        let revoked = state
            .db
            .tokens()
            .delete_by_jti(&jti)
            .await
            .db_err("Failed to revoke token")?;

        Ok((StatusCode::OK, Json(RevokeResponse { revoked })))
    } else {
        // Token not found - already revoked or never existed
        Ok((StatusCode::OK, Json(RevokeResponse { revoked: false })))
    }
}
