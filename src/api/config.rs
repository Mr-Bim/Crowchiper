//! Public configuration endpoint.

use axum::{Json, Router, extract::State, routing::get};
use serde::Serialize;
use std::sync::Arc;

use crate::auth::{HasAuthState, MaybeAuth};
use crate::cli::ClientIpHeader;
use crate::db::Database;
use crate::jwt::JwtConfig;

#[derive(Clone)]
pub struct ConfigState {
    pub no_signup: bool,
    pub jwt: Arc<JwtConfig>,
    pub db: Database,
    pub secure_cookies: bool,
    pub ip_header: Option<ClientIpHeader>,
}

impl HasAuthState for ConfigState {
    fn jwt(&self) -> &JwtConfig {
        &self.jwt
    }

    fn db(&self) -> &Database {
        &self.db
    }

    fn secure_cookies(&self) -> bool {
        self.secure_cookies
    }

    fn ip_header(&self) -> Option<&ClientIpHeader> {
        self.ip_header.as_ref()
    }
}

#[derive(Serialize)]
struct ConfigResponse {
    no_signup: bool,
    authenticated: bool,
}

pub fn router(state: ConfigState) -> Router {
    Router::new().route("/", get(get_config)).with_state(state)
}

async fn get_config(
    State(state): State<ConfigState>,
    MaybeAuth(user): MaybeAuth,
) -> Json<ConfigResponse> {
    Json(ConfigResponse {
        no_signup: state.no_signup,
        authenticated: user.is_some(),
    })
}
