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
