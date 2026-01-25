//! Authentication user types.

use crate::jwt::AccessClaims;

/// Authenticated user information extracted from JWT.
#[derive(Debug, Clone)]
pub struct AuthenticatedUser {
    /// JWT claims from the access token
    pub claims: AccessClaims,
    /// Database user ID (only set when token was refreshed)
    pub user_id: Option<i64>,
}

/// Authenticated user with guaranteed database user ID.
/// Used for endpoints that require an activated account.
#[derive(Debug, Clone)]
pub struct ActivatedAuthenticatedUser {
    /// JWT claims from the access token
    pub claims: AccessClaims,
    /// Database user ID
    pub user_id: i64,
}
