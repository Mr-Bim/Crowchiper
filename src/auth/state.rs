//! Authentication state traits and macro.

use crate::cli::IpExtractor;
use crate::db::Database;
use crate::jwt::JwtConfig;

/// Trait for state types that support API authentication.
pub trait HasAuthState {
    fn jwt(&self) -> &JwtConfig;
    fn db(&self) -> &Database;
    fn secure_cookies(&self) -> bool;
    fn ip_extractor(&self) -> Option<&IpExtractor>;
}

/// Trait for state types that support asset authentication.
/// Extends `HasAuthState` with the login path for redirects.
pub trait HasAssetAuthState: HasAuthState {
    fn login_path(&self) -> &str;
}

/// Macro to implement `HasAuthState` for state structs with the standard fields.
///
/// The struct must have these fields:
/// - `jwt: Arc<JwtConfig>`
/// - `db: Database`
/// - `secure_cookies: bool`
/// - `ip_extractor: Option<IpExtractor>`
///
/// # Example
/// ```ignore
/// use crate::impl_has_auth_state;
///
/// #[derive(Clone)]
/// pub struct MyState {
///     pub db: Database,
///     pub jwt: Arc<JwtConfig>,
///     pub secure_cookies: bool,
///     pub ip_extractor: Option<IpExtractor>,
///     // ... other fields
/// }
///
/// impl_has_auth_state!(MyState);
/// ```
#[macro_export]
macro_rules! impl_has_auth_state {
    ($state_type:ty) => {
        impl $crate::auth::HasAuthState for $state_type {
            fn jwt(&self) -> &$crate::jwt::JwtConfig {
                &self.jwt
            }
            fn db(&self) -> &$crate::db::Database {
                &self.db
            }
            fn secure_cookies(&self) -> bool {
                self.secure_cookies
            }
            fn ip_extractor(&self) -> Option<&$crate::cli::IpExtractor> {
                self.ip_extractor.as_ref()
            }
        }
    };
}
