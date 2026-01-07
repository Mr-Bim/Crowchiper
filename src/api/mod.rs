mod attachments;
mod config;
mod encryption;
mod error;
mod passkeys;
mod posts;
mod users;

use axum::Router;
use std::sync::Arc;
use webauthn_rs::prelude::*;

use crate::db::Database;
use crate::jwt::JwtConfig;

pub use users::UsersState;

/// Create the API router.
pub fn create_api_router(
    db: Database,
    webauthn: Arc<Webauthn>,
    jwt: Arc<JwtConfig>,
    secure_cookies: bool,
    no_signup: bool,
) -> Router {
    let passkeys_state = passkeys::PasskeysState {
        db: db.clone(),
        webauthn,
        jwt: jwt.clone(),
        secure_cookies,
    };

    let posts_state = posts::PostsState {
        db: db.clone(),
        jwt: jwt.clone(),
    };

    let encryption_state = encryption::EncryptionState {
        db: db.clone(),
        jwt: jwt.clone(),
    };

    let attachments_state = attachments::AttachmentsState {
        db: db.clone(),
        jwt: jwt.clone(),
    };

    let users_state = users::UsersState { db, jwt, no_signup };

    let config_state = config::ConfigState { no_signup };

    Router::new()
        .nest("/users", users::router(users_state))
        .nest("/passkeys", passkeys::router(passkeys_state))
        .nest("/posts", posts::router(posts_state))
        .nest("/encryption", encryption::router(encryption_state))
        .nest("/config", config::router(config_state))
        .nest("/attachments", attachments::router(attachments_state))
}
