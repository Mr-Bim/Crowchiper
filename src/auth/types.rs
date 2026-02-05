//! Authentication user types.

use crate::jwt::AccessClaims;

/// Authenticated user information extracted from JWT.
/// May not have database ID if token was valid without refresh.
#[derive(Debug, Clone)]
pub struct AuthenticatedUser {
    pub claims: AccessClaims,
    /// Only set when token was refreshed via refresh token.
    pub user_id: Option<i64>,
    /// Only set when token was refreshed via refresh token.
    pub refresh_jti: Option<String>,
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

/// Authenticated user with session information (refresh token JTI).
/// Used for endpoints that need to identify the current session,
/// such as token listing or revocation.
#[derive(Debug, Clone)]
pub struct AuthenticatedUserWithSession {
    /// JWT claims from the access token
    pub claims: AccessClaims,
    /// Database user ID
    pub user_id: i64,
    /// Refresh token JTI (identifies the current session)
    pub refresh_jti: String,
}
