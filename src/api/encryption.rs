//! Encryption settings API.
//!
//! All endpoints require JWT authentication.

use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
};
use rand::RngCore;
use serde::Serialize;
use std::sync::Arc;

use super::error::{ApiError, ResultExt};
use crate::auth::{ActivatedApiAuth, HasAuthState};
use crate::db::Database;
use crate::jwt::JwtConfig;

/// State for encryption endpoints.
#[derive(Clone)]
pub struct EncryptionState {
    pub db: Database,
    pub jwt: Arc<JwtConfig>,
    pub secure_cookies: bool,
}

impl HasAuthState for EncryptionState {
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

pub fn router(state: EncryptionState) -> Router {
    Router::new()
        .route("/settings", get(get_settings))
        .route("/setup", post(setup_encryption))
        .route("/skip", post(skip_encryption))
        .with_state(state)
}

// --- Response types ---

#[derive(Serialize)]
struct SettingsResponse {
    setup_done: bool,
    encryption_enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    prf_salt: Option<String>,
}

#[derive(Serialize)]
struct SetupResponse {
    prf_salt: String, // base64
}

// --- Handlers ---

/// Get encryption settings for the current user.
async fn get_settings(
    State(state): State<EncryptionState>,
    ActivatedApiAuth(user): ActivatedApiAuth,
) -> Result<impl IntoResponse, ApiError> {
    let settings = state
        .db
        .encryption_settings()
        .get(user.user_id)
        .await
        .db_err("Failed to get encryption settings")?;

    let response = match settings {
        Some(s) => SettingsResponse {
            setup_done: s.setup_done,
            encryption_enabled: s.encryption_enabled,
            prf_salt: s.prf_salt.map(|salt| base64_encode(&salt)),
        },
        None => SettingsResponse {
            setup_done: false,
            encryption_enabled: false,
            prf_salt: None,
        },
    };

    Ok(Json(response))
}

/// Set up encryption for the current user.
/// Generates a random PRF salt server-side and returns it.
async fn setup_encryption(
    State(state): State<EncryptionState>,
    ActivatedApiAuth(user): ActivatedApiAuth,
) -> Result<impl IntoResponse, ApiError> {
    // Check if already set up
    let existing = state
        .db
        .encryption_settings()
        .get(user.user_id)
        .await
        .db_err("Failed to check existing settings")?;

    if existing.is_some() {
        return Err(ApiError::conflict("Encryption already set up"));
    }

    // Generate random 32-byte PRF salt
    let mut prf_salt = [0u8; 32];
    rand::rng().fill_bytes(&mut prf_salt);

    state
        .db
        .encryption_settings()
        .create(user.user_id, &prf_salt)
        .await
        .db_err("Failed to save encryption settings")?;

    Ok((
        StatusCode::CREATED,
        Json(SetupResponse {
            prf_salt: base64_encode(&prf_salt),
        }),
    ))
}

/// Skip encryption setup (PRF not supported).
/// Marks setup as done without enabling encryption.
async fn skip_encryption(
    State(state): State<EncryptionState>,
    ActivatedApiAuth(user): ActivatedApiAuth,
) -> Result<impl IntoResponse, ApiError> {
    // Check if already set up
    let existing = state
        .db
        .encryption_settings()
        .get(user.user_id)
        .await
        .db_err("Failed to check existing settings")?;

    if existing.is_some() {
        return Err(ApiError::conflict("Encryption already set up"));
    }

    state
        .db
        .encryption_settings()
        .mark_setup_done(user.user_id)
        .await
        .db_err("Failed to save encryption settings")?;

    Ok(StatusCode::NO_CONTENT)
}

// --- Helper functions ---

fn base64_encode(data: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(data)
}
