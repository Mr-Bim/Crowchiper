//! Authentication state traits and macro.

use std::sync::Arc;

use crate::cli::IpExtractor;
use crate::db::Database;
use crate::jwt::JwtConfig;
use crate::plugin::PluginManager;

/// Server-level settings shared across all state structs via `Arc`.
#[derive(Clone)]
pub struct ServerSettings {
    pub ip_extractor: Option<IpExtractor>,
    pub secure_cookies: bool,
    pub plugin_manager: Option<Arc<PluginManager>>,
}

/// Trait for state types that provide database, JWT, and server settings for authentication.
pub trait HasAuthBackend {
    fn jwt(&self) -> &JwtConfig;
    fn db(&self) -> &Database;
    fn ip_extractor(&self) -> Option<&IpExtractor>;
    fn secure_cookies(&self) -> bool;
    fn plugin_manager(&self) -> Option<&Arc<PluginManager>>;
}

/// Trait for state types that support asset authentication.
/// Extends `HasAuthBackend` with the login path for redirects.
pub trait HasAssetAuthBackend: HasAuthBackend {
    fn login_path(&self) -> &str;
}

/// Macro to implement `HasAuthBackend` for state structs with the standard fields.
///
/// The struct must have these fields:
/// - `jwt: Arc<JwtConfig>`
/// - `db: Database`
/// - `settings: Arc<ServerSettings>`
///
/// # Example
/// ```ignore
/// use crate::impl_has_auth_backend;
///
/// #[derive(Clone)]
/// pub struct MyState {
///     pub db: Database,
///     pub jwt: Arc<JwtConfig>,
///     pub settings: Arc<ServerSettings>,
///     // ... other fields
/// }
///
/// impl_has_auth_backend!(MyState);
/// ```
#[macro_export]
macro_rules! impl_has_auth_backend {
    ($state_type:ty) => {
        impl $crate::auth::HasAuthBackend for $state_type {
            fn jwt(&self) -> &$crate::jwt::JwtConfig {
                &self.jwt
            }
            fn db(&self) -> &$crate::db::Database {
                &self.db
            }
            fn ip_extractor(&self) -> Option<&$crate::cli::IpExtractor> {
                self.settings.ip_extractor.as_ref()
            }
            fn secure_cookies(&self) -> bool {
                self.settings.secure_cookies
            }
            fn plugin_manager(&self) -> Option<&std::sync::Arc<$crate::plugin::PluginManager>> {
                self.settings.plugin_manager.as_ref()
            }
        }
    };
}
