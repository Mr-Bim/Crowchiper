//! Encryption mutation API (setup/skip).
//!
//! The GET settings endpoint has moved to `/api/user/settings` (user_settings module).

use axum::{Json, Router, extract::State, http::StatusCode, response::IntoResponse, routing::post};
use rand::RngCore;
use serde::Serialize;
use std::sync::Arc;

use super::error::{ApiError, ResultExt};
use crate::auth::{AnyRole, Auth};
use crate::db::Database;
use crate::impl_has_auth_backend;
use crate::jwt::JwtConfig;

/// State for encryption endpoints.
#[derive(Clone)]
pub struct EncryptionState {
    pub db: Database,
    pub jwt: Arc<JwtConfig>,
}

impl_has_auth_backend!(EncryptionState);

pub fn router(state: EncryptionState) -> Router {
    Router::new()
        .route("/setup", post(setup_encryption))
        .route("/skip", post(skip_encryption))
        .with_state(state)
}

// --- Response types ---

#[derive(Serialize)]
struct SetupResponse {
    prf_salt: String, // base64
}

// --- Handlers ---

/// Set up encryption for the current user.
/// Generates a random PRF salt server-side and returns it.
async fn setup_encryption(
    State(state): State<EncryptionState>,
    auth: Auth<AnyRole>,
) -> Result<impl IntoResponse, ApiError> {
    // Check if already set up
    let existing = state
        .db
        .encryption_settings()
        .get(auth.user_id)
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
        .create(auth.user_id, &prf_salt)
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
    auth: Auth<AnyRole>,
) -> Result<impl IntoResponse, ApiError> {
    // Check if already set up
    let existing = state
        .db
        .encryption_settings()
        .get(auth.user_id)
        .await
        .db_err("Failed to check existing settings")?;

    if existing.is_some() {
        return Err(ApiError::conflict("Encryption already set up"));
    }

    state
        .db
        .encryption_settings()
        .mark_setup_done(auth.user_id)
        .await
        .db_err("Failed to save encryption settings")?;

    Ok(StatusCode::NO_CONTENT)
}

// --- Helper functions ---

fn base64_encode(data: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(data)
}
