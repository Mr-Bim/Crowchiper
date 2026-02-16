//! Public configuration endpoint.

use axum::{Json, Router, extract::State, routing::get};
use serde::Serialize;
use std::sync::Arc;

use crate::auth::{OptionalAuth, ServerSettings};
use crate::db::Database;
use crate::impl_has_auth_backend;
use crate::jwt::JwtConfig;

/// Version embedded at compile time from Cargo.toml
const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Git commit hash embedded at compile time
const GIT_COMMIT_HASH: &str = env!("GIT_COMMIT_HASH");

#[derive(Clone)]
pub struct ConfigState {
    pub no_signup: bool,
    pub jwt: Arc<JwtConfig>,
    pub db: Database,
    pub settings: Arc<ServerSettings>,
}

impl_has_auth_backend!(ConfigState);

#[derive(Serialize)]
struct ConfigResponse {
    no_signup: bool,
    authenticated: bool,
    version: &'static str,
    git_hash: &'static str,
}

pub fn router(state: ConfigState) -> Router {
    Router::new().route("/", get(get_config)).with_state(state)
}

async fn get_config(
    State(state): State<ConfigState>,
    OptionalAuth(user): OptionalAuth,
) -> Json<ConfigResponse> {
    Json(ConfigResponse {
        no_signup: state.no_signup,
        authenticated: user.is_some(),
        version: VERSION,
        git_hash: GIT_COMMIT_HASH,
    })
}
