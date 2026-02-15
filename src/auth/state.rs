//! Authentication state traits and macro.

use crate::db::Database;
use crate::jwt::JwtConfig;

/// Trait for state types that provide database and JWT access for authentication.
pub trait HasAuthBackend {
    fn jwt(&self) -> &JwtConfig;
    fn db(&self) -> &Database;
}

/// Trait for state types that support asset authentication.
/// Extends `HasDbJwt` with the login path for redirects.
pub trait HasAssetAuthBackend: HasAuthBackend {
    fn login_path(&self) -> &str;
}

/// Macro to implement `HasDbJwt` for state structs with the standard fields.
///
/// The struct must have these fields:
/// - `jwt: Arc<JwtConfig>`
/// - `db: Database`
///
/// # Example
/// ```ignore
/// use crate::impl_has_db_jwt;
///
/// #[derive(Clone)]
/// pub struct MyState {
///     pub db: Database,
///     pub jwt: Arc<JwtConfig>,
///     // ... other fields
/// }
///
/// impl_has_db_jwt!(MyState);
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
        }
    };
}
