//! JWT authentication with role-based access control.
//!
//! Dual-token system: short-lived access tokens (5 min, stateless) and
//! long-lived refresh tokens (2 weeks, database-tracked). Access tokens
//! are automatically refreshed via middleware when expired.

mod cookie;
mod errors;
mod extractors;
mod ip;
mod state;
mod types;

pub use cookie::{ACCESS_COOKIE_NAME, REFRESH_COOKIE_NAME, get_cookie};
pub use errors::{ApiAuthError, AssetAuthError};
pub use extractors::{
    AdminOnly, AnyRole, Auth, AuthWithSession, NEW_ACCESS_TOKEN_COOKIE, OptionalAuth,
    ProtectedAsset, RoleConstraint, add_access_token_cookie,
};
pub use ip::{HasHeadersAndExtensions, extract_client_ip};
pub use state::{HasAssetAuthBackend, HasAuthBackend, ServerSettings};
pub use types::{ActivatedAuthenticatedUser, AuthenticatedUser, AuthenticatedUserWithSession};
