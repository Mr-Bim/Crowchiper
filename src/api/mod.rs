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

use crate::cli::IpExtractor;
use crate::db::Database;
use crate::jwt::JwtConfig;
use crate::plugin::PluginManager;
use crate::rate_limit::RateLimitConfig;

pub use users::UsersState;

/// Create the API router.
pub fn create_api_router(
    db: Database,
    webauthn: Arc<Webauthn>,
    jwt: Arc<JwtConfig>,
    secure_cookies: bool,
    no_signup: bool,
    ip_extractor: Option<IpExtractor>,
    plugin_manager: Option<Arc<PluginManager>>,
    dashboard_path: &'static str,
) -> Router {
    let rate_limit_config = Arc::new(RateLimitConfig::new(ip_extractor.clone()));

    let passkeys_state = passkeys::PasskeysState {
        db: db.clone(),
        webauthn,
        jwt: jwt.clone(),
        secure_cookies,
        ip_extractor: ip_extractor.clone(),
    };

    let posts_state = posts::PostsState {
        db: db.clone(),
        jwt: jwt.clone(),
        secure_cookies,
        ip_extractor: ip_extractor.clone(),
        plugin_manager: plugin_manager.clone(),
    };

    let encryption_state = encryption::EncryptionState {
        db: db.clone(),
        jwt: jwt.clone(),
        secure_cookies,
        ip_extractor: ip_extractor.clone(),
        plugin_manager: plugin_manager.clone(),
    };

    let attachments_state = attachments::AttachmentsState {
        db: db.clone(),
        jwt: jwt.clone(),
        secure_cookies,
        ip_extractor: ip_extractor.clone(),
        plugin_manager: plugin_manager.clone(),
    };

    let tokens_state = tokens::TokensState {
        db: db.clone(),
        jwt: jwt.clone(),
        secure_cookies,
        ip_extractor: ip_extractor.clone(),
        plugin_manager: plugin_manager.clone(),
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
        secure_cookies,
        ip_extractor: ip_extractor.clone(),
        plugin_manager: plugin_manager.clone(),
    };

    let admin_state = admin::AdminState {
        db: db.clone(),
        jwt: jwt.clone(),
        secure_cookies,
        ip_extractor: ip_extractor.clone(),
        plugin_manager: plugin_manager.clone(),
    };

    let user_settings_state = user_settings::UserSettingsState {
        db: db.clone(),
        jwt: jwt.clone(),
        secure_cookies,
        ip_extractor: ip_extractor.clone(),
        plugin_manager: plugin_manager.clone(),
        dashboard_path,
    };

    let users_state = users::UsersState {
        db,
        jwt,
        secure_cookies,
        ip_extractor,
        plugin_manager,
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
