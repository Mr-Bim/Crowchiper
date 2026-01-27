//! JWT authentication for API routes.
//!
//! Uses a dual-token system:
//! - Access tokens: Short-lived (5 min), stateless, no database check
//! - Refresh tokens: Long-lived (2 weeks), tracked in database with JTI
//!
//! When an access token expires, the middleware automatically issues a new one
//! if the refresh token is still valid (not expired, not revoked).
//!
//! # Module Structure
//!
//! - `cookie`: Cookie parsing utilities
//! - `errors`: Authentication error types
//! - `extractors`: Axum extractors for authentication
//! - `ip`: Client IP extraction
//! - `state`: Traits for state types that support authentication
//! - `types`: Authenticated user types

mod cookie;
mod errors;
mod extractors;
mod ip;
mod state;
mod types;

// Re-export all public items for backward compatibility
pub use cookie::{ACCESS_COOKIE_NAME, REFRESH_COOKIE_NAME, get_cookie};
pub use errors::{ApiAuthError, AssetAuthError};
pub use extractors::{
    ActivatedApiAuth, ActivatedApiAuthWithJti, ApiAuth, AssetAuth, MaybeAuth,
    NEW_ACCESS_TOKEN_COOKIE, NewAccessTokenCookie,
};
pub use ip::{HasHeadersAndExtensions, extract_client_ip};
pub use state::{HasAssetAuthState, HasAuthState};
pub use types::{ActivatedAuthenticatedUser, ActivatedAuthenticatedUserWithJti, AuthenticatedUser};
