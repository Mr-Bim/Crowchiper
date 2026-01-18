//! Test-mode only API endpoints.
//!
//! These endpoints are only available when compiled with the `test-mode` feature.
//! They provide functionality needed for e2e testing that shouldn't be exposed in production.

use axum::{Json, Router, extract::State, http::StatusCode, response::IntoResponse, routing::post};
use serde::{Deserialize, Serialize};

use super::error::{ApiError, ResultExt};
use crate::db::Database;

#[derive(Clone)]
pub struct TestState {
    pub db: Database,
}

pub fn router(state: TestState) -> Router {
    Router::new()
        .route("/admin", post(create_admin_user))
        .route("/token", post(store_token))
        .with_state(state)
}

#[derive(Deserialize)]
struct CreateAdminRequest {
    username: String,
}

#[derive(Serialize)]
struct CreateAdminResponse {
    uuid: String,
    username: String,
}

/// Create an admin user for testing claim flows.
async fn create_admin_user(
    State(state): State<TestState>,
    Json(payload): Json<CreateAdminRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let username = payload.username.trim();

    if username.is_empty() {
        return Err(ApiError::bad_request("Username cannot be empty"));
    }

    if username.len() > 32 {
        return Err(ApiError::bad_request(
            "Username cannot be longer than 32 characters",
        ));
    }

    let uuid = uuid::Uuid::new_v4().to_string();

    state
        .db
        .users()
        .create_admin(&uuid, username)
        .await
        .db_err("Failed to create admin user")?;

    Ok((
        StatusCode::CREATED,
        Json(CreateAdminResponse {
            uuid,
            username: username.to_string(),
        }),
    ))
}

#[derive(Deserialize)]
struct StoreTokenRequest {
    jti: String,
    user_uuid: String,
    username: String,
    issued_at: u64,
    expires_at: u64,
}

#[derive(Serialize)]
struct StoreTokenResponse {
    success: bool,
}

/// Store a token in the database for e2e testing.
/// This allows tests to manually create JWT tokens and register them.
async fn store_token(
    State(state): State<TestState>,
    Json(payload): Json<StoreTokenRequest>,
) -> Result<impl IntoResponse, ApiError> {
    // First, ensure the user exists and is activated
    let user = state
        .db
        .users()
        .get_by_uuid(&payload.user_uuid)
        .await
        .db_err("Failed to get user")?;

    let user_id = if let Some(user) = user {
        user.id
    } else {
        // Create the user if it doesn't exist
        let id = state
            .db
            .users()
            .create(&payload.user_uuid, &payload.username)
            .await
            .db_err("Failed to create user")?;
        state
            .db
            .users()
            .activate(id)
            .await
            .db_err("Failed to activate user")?;
        id
    };

    // Store the token
    state
        .db
        .tokens()
        .create(
            &payload.jti,
            user_id,
            None,
            payload.issued_at,
            payload.expires_at,
        )
        .await
        .db_err("Failed to store token")?;

    Ok((StatusCode::OK, Json(StoreTokenResponse { success: true })))
}
