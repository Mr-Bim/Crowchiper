//! Public configuration endpoint.

use axum::{Json, Router, extract::State, routing::get};
use serde::Serialize;

#[derive(Clone)]
pub struct ConfigState {
    pub no_signup: bool,
}

#[derive(Serialize)]
struct ConfigResponse {
    no_signup: bool,
}

pub fn router(state: ConfigState) -> Router {
    Router::new().route("/", get(get_config)).with_state(state)
}

async fn get_config(State(state): State<ConfigState>) -> Json<ConfigResponse> {
    Json(ConfigResponse {
        no_signup: state.no_signup,
    })
}
