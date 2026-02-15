//! User settings API.
//!
//! Combines encryption settings with user-specific info (admin status, dashboard path).

use axum::{Json, Router, extract::State, response::IntoResponse, routing::get};
use serde::Serialize;
use std::sync::Arc;

use super::error::{ApiError, ResultExt};
use crate::auth::{AnyRole, Auth};
use crate::db::{Database, UserRole};
use crate::impl_has_auth_backend;
use crate::jwt::JwtConfig;

/// State for user settings endpoint.
#[derive(Clone)]
pub struct UserSettingsState {
    pub db: Database,
    pub jwt: Arc<JwtConfig>,
    pub dashboard_path: &'static str,
}

impl_has_auth_backend!(UserSettingsState);

pub fn router(state: UserSettingsState) -> Router {
    Router::new()
        .route("/settings", get(get_settings))
        .with_state(state)
}

#[derive(Serialize)]
struct UserSettingsResponse {
    setup_done: bool,
    encryption_enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    prf_salt: Option<String>,
    is_admin: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    dashboard_path: Option<String>,
}

/// Get user settings: encryption status + admin info.
async fn get_settings(
    State(state): State<UserSettingsState>,
    auth: Auth<AnyRole>,
) -> Result<impl IntoResponse, ApiError> {
    let settings = state
        .db
        .encryption_settings()
        .get(auth.user_id)
        .await
        .db_err("Failed to get encryption settings")?;

    let is_admin = auth.claims.role == UserRole::Admin;

    let (setup_done, encryption_enabled, prf_salt) = match settings {
        Some(s) => (
            s.setup_done,
            s.encryption_enabled,
            s.prf_salt.map(|salt| base64_encode(&salt)),
        ),
        None => (false, false, None),
    };

    Ok(Json(UserSettingsResponse {
        setup_done,
        encryption_enabled,
        prf_salt,
        is_admin,
        dashboard_path: if is_admin {
            Some(state.dashboard_path.to_string())
        } else {
            None
        },
    }))
}

fn base64_encode(data: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(data)
}
