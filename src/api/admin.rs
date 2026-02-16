//! Admin API endpoints.
//!
//! All endpoints require admin role.

use axum::{Json, Router, extract::State, response::IntoResponse, routing::get};
use std::sync::Arc;

use super::error::{ApiError, ResultExt};
use crate::auth::{AdminOnly, Auth, ServerSettings};
use crate::db::Database;
use crate::impl_has_auth_backend;
use crate::jwt::JwtConfig;

/// State for admin endpoints.
#[derive(Clone)]
pub struct AdminState {
    pub db: Database,
    pub jwt: Arc<JwtConfig>,
    pub settings: Arc<ServerSettings>,
}

impl_has_auth_backend!(AdminState);

pub fn router(state: AdminState) -> Router {
    Router::new()
        .route("/users", get(list_users))
        .with_state(state)
}

/// List all activated users.
async fn list_users(
    State(state): State<AdminState>,
    _auth: Auth<AdminOnly>,
) -> Result<impl IntoResponse, ApiError> {
    let users = state
        .db
        .users()
        .list_activated()
        .await
        .db_err("Failed to list users")?;

    Ok(Json(users))
}
