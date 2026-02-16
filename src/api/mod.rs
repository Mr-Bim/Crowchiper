mod admin;
mod attachments;
mod config;
mod encryption;
mod error;
mod passkeys;
mod posts;
#[cfg(feature = "test-mode")]
mod test;
mod tokens;
mod user_settings;
mod users;

use axum::Router;
use std::sync::Arc;
use webauthn_rs::prelude::*;

use crate::auth::ServerSettings;
use crate::db::Database;
use crate::jwt::JwtConfig;
use crate::rate_limit::RateLimitConfig;

pub use users::UsersState;

/// Create the API router.
pub fn create_api_router(
    db: Database,
    webauthn: Arc<Webauthn>,
    jwt: Arc<JwtConfig>,
    no_signup: bool,
    dashboard_path: &'static str,
    settings: Arc<ServerSettings>,
) -> Router {
    let rate_limit_config = Arc::new(RateLimitConfig::new(settings.ip_extractor.clone()));

    let passkeys_state = passkeys::PasskeysState {
        db: db.clone(),
        webauthn,
        jwt: jwt.clone(),
        settings: settings.clone(),
    };

    let posts_state = posts::PostsState {
        db: db.clone(),
        jwt: jwt.clone(),
        settings: settings.clone(),
    };

    let encryption_state = encryption::EncryptionState {
        db: db.clone(),
        jwt: jwt.clone(),
        settings: settings.clone(),
    };

    let attachments_state = attachments::AttachmentsState {
        db: db.clone(),
        jwt: jwt.clone(),
        settings: settings.clone(),
    };

    let tokens_state = tokens::TokensState {
        db: db.clone(),
        jwt: jwt.clone(),
        settings: settings.clone(),
    };

    #[cfg(feature = "test-mode")]
    let test_state = test::TestState {
        db: db.clone(),
        jwt: jwt.clone(),
    };

    let config_state = config::ConfigState {
        no_signup,
        jwt: jwt.clone(),
        db: db.clone(),
        settings: settings.clone(),
    };

    let admin_state = admin::AdminState {
        db: db.clone(),
        jwt: jwt.clone(),
        settings: settings.clone(),
    };

    let user_settings_state = user_settings::UserSettingsState {
        db: db.clone(),
        jwt: jwt.clone(),
        settings: settings.clone(),
        dashboard_path,
    };

    let users_state = users::UsersState {
        db,
        jwt,
        settings,
        no_signup,
        rate_limit_config: rate_limit_config.clone(),
    };

    let router = Router::new()
        .nest("/users", users::router(users_state))
        .nest(
            "/passkeys",
            passkeys::router(passkeys_state, rate_limit_config),
        )
        .nest("/posts", posts::router(posts_state))
        .nest("/encryption", encryption::router(encryption_state))
        .nest("/config", config::router(config_state))
        .nest("/attachments", attachments::router(attachments_state))
        .nest("/tokens", tokens::router(tokens_state))
        .nest("/admin", admin::router(admin_state))
        .nest("/user", user_settings::router(user_settings_state));

    #[cfg(feature = "test-mode")]
    let router = router.nest("/test", test::router(test_state));

    router
}
