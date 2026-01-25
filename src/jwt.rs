//! JWT token generation and validation.

use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::db::UserRole;

/// Token type for distinguishing access vs refresh tokens.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TokenType {
    /// Short-lived access token (5 minutes) - stateless, no JTI
    Access,
    /// Long-lived refresh token (2 weeks) - tracked in database with JTI
    Refresh,
}

/// JWT claims for access tokens (stateless, no JTI).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessClaims {
    /// Subject (user UUID)
    pub sub: String,
    /// Username
    pub username: String,
    /// User role
    pub role: UserRole,
    /// Token type
    #[serde(rename = "typ")]
    pub token_type: TokenType,
    /// Issued at (Unix timestamp)
    pub iat: u64,
    /// Expiration time (Unix timestamp)
    pub exp: u64,
    /// Ip address
    pub ipaddr: String,
}

/// JWT claims for refresh tokens (tracked with JTI).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefreshClaims {
    /// JWT ID (unique identifier for revocation tracking)
    pub jti: String,
    /// Subject (user UUID)
    pub sub: String,
    /// Username
    pub username: String,
    /// User role
    pub role: UserRole,
    /// Token type
    #[serde(rename = "typ")]
    pub token_type: TokenType,
    /// Issued at (Unix timestamp)
    pub iat: u64,
    /// Expiration time (Unix timestamp)
    pub exp: u64,
}

/// Access token duration: 5 minutes
pub const ACCESS_TOKEN_DURATION_SECS: u64 = 5 * 60;

/// Refresh token duration: 2 weeks
pub const REFRESH_TOKEN_DURATION_SECS: u64 = 14 * 24 * 60 * 60;

/// Configuration for JWT operations.
#[derive(Clone)]
pub struct JwtConfig {
    encoding_key: EncodingKey,
    decoding_key: DecodingKey,
}

/// Result of generating an access token (no JTI).
#[derive(Debug, Clone)]
pub struct AccessTokenResult {
    /// The JWT token string
    pub token: String,
    /// Token duration in seconds
    pub duration: u64,
}

/// Result of generating a refresh token (with JTI for tracking).
#[derive(Debug, Clone)]
pub struct RefreshTokenResult {
    /// The JWT token string
    pub token: String,
    /// JWT ID (unique identifier for database tracking)
    pub jti: String,
    /// Issued at timestamp (Unix seconds)
    pub issued_at: u64,
    /// Expiration timestamp (Unix seconds)
    pub expires_at: u64,
    /// Token duration in seconds
    pub duration: u64,
}

impl JwtConfig {
    /// Create a new JWT configuration with the given secret.
    pub fn new(secret: &[u8]) -> Self {
        Self {
            encoding_key: EncodingKey::from_secret(secret),
            decoding_key: DecodingKey::from_secret(secret),
        }
    }

    /// Generate an access token for a user.
    /// Access tokens are short-lived (5 minutes), stateless, and have no JTI.
    pub fn generate_access_token(
        &self,
        user_uuid: &str,
        username: &str,
        role: UserRole,
        ip_addr: &str,
    ) -> Result<AccessTokenResult, JwtError> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| JwtError::TimeError)?
            .as_secs();

        let exp = now + ACCESS_TOKEN_DURATION_SECS;

        let claims = AccessClaims {
            sub: user_uuid.to_string(),
            username: username.to_string(),
            role,
            token_type: TokenType::Access,
            iat: now,
            exp,
            ipaddr: ip_addr.to_string(),
        };

        let token = jsonwebtoken::encode(&Header::default(), &claims, &self.encoding_key)
            .map_err(JwtError::Encoding)?;

        Ok(AccessTokenResult {
            token,
            duration: ACCESS_TOKEN_DURATION_SECS,
        })
    }

    /// Generate a refresh token for a user.
    /// Refresh tokens are long-lived (2 weeks) and tracked in the database with JTI.
    pub fn generate_refresh_token(
        &self,
        user_uuid: &str,
        username: &str,
        role: UserRole,
    ) -> Result<RefreshTokenResult, JwtError> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| JwtError::TimeError)?
            .as_secs();

        let jti = uuid::Uuid::new_v4().to_string();
        let exp = now + REFRESH_TOKEN_DURATION_SECS;

        let claims = RefreshClaims {
            jti: jti.clone(),
            sub: user_uuid.to_string(),
            username: username.to_string(),
            role,
            token_type: TokenType::Refresh,
            iat: now,
            exp,
        };

        let token = jsonwebtoken::encode(&Header::default(), &claims, &self.encoding_key)
            .map_err(JwtError::Encoding)?;

        Ok(RefreshTokenResult {
            token,
            jti,
            issued_at: now,
            expires_at: exp,
            duration: REFRESH_TOKEN_DURATION_SECS,
        })
    }

    /// Validate and decode an access token.
    pub fn validate_access_token(&self, token: &str) -> Result<AccessClaims, JwtError> {
        let mut validation = Validation::new(Algorithm::HS256);
        validation.leeway = 0;

        let token_data =
            jsonwebtoken::decode::<AccessClaims>(token, &self.decoding_key, &validation)
                .map_err(JwtError::Decoding)?;

        if token_data.claims.token_type != TokenType::Access {
            return Err(JwtError::WrongTokenType);
        }

        Ok(token_data.claims)
    }

    /// Validate and decode a refresh token.
    pub fn validate_refresh_token(&self, token: &str) -> Result<RefreshClaims, JwtError> {
        let mut validation = Validation::new(Algorithm::HS256);
        validation.leeway = 0;

        let token_data =
            jsonwebtoken::decode::<RefreshClaims>(token, &self.decoding_key, &validation)
                .map_err(JwtError::Decoding)?;

        if token_data.claims.token_type != TokenType::Refresh {
            return Err(JwtError::WrongTokenType);
        }

        Ok(token_data.claims)
    }
}

/// Errors that can occur during JWT operations.
#[derive(Debug)]
pub enum JwtError {
    /// Error encoding the token
    Encoding(jsonwebtoken::errors::Error),
    /// Error decoding the token
    Decoding(jsonwebtoken::errors::Error),
    /// System time error
    TimeError,
    /// Wrong token type (e.g., using refresh token as access token)
    WrongTokenType,
}

impl std::fmt::Display for JwtError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            JwtError::Encoding(e) => write!(f, "Failed to encode token: {}", e),
            JwtError::Decoding(e) => write!(f, "Failed to decode token: {}", e),
            JwtError::TimeError => write!(f, "System time error"),
            JwtError::WrongTokenType => write!(f, "Wrong token type"),
        }
    }
}

impl std::error::Error for JwtError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_and_validate_access_token() {
        let config = JwtConfig::new(b"test-secret-key-for-testing");

        let result = config
            .generate_access_token("uuid-123", "alice", UserRole::User, "ip")
            .unwrap();

        assert_eq!(result.duration, ACCESS_TOKEN_DURATION_SECS);

        let claims = config.validate_access_token(&result.token).unwrap();
        assert_eq!(claims.sub, "uuid-123");
        assert_eq!(claims.username, "alice");
        assert_eq!(claims.role, UserRole::User);
        assert_eq!(claims.token_type, TokenType::Access);
        assert_eq!(claims.ipaddr, "ip")
    }

    #[test]
    fn test_generate_and_validate_refresh_token() {
        let config = JwtConfig::new(b"test-secret-key-for-testing");

        let result = config
            .generate_refresh_token("uuid-123", "alice", UserRole::User)
            .unwrap();

        assert_eq!(result.duration, REFRESH_TOKEN_DURATION_SECS);
        assert!(!result.jti.is_empty());

        let claims = config.validate_refresh_token(&result.token).unwrap();
        assert_eq!(claims.sub, "uuid-123");
        assert_eq!(claims.username, "alice");
        assert_eq!(claims.role, UserRole::User);
        assert_eq!(claims.token_type, TokenType::Refresh);
        assert_eq!(claims.jti, result.jti);
    }

    #[test]
    fn test_wrong_token_type_rejected() {
        let config = JwtConfig::new(b"test-secret-key-for-testing");

        let access = config
            .generate_access_token("uuid-123", "alice", UserRole::User, "ip")
            .unwrap();

        let refresh = config
            .generate_refresh_token("uuid-123", "alice", UserRole::User)
            .unwrap();

        // Access token should fail validate_refresh_token
        assert!(config.validate_refresh_token(&access.token).is_err());

        // Refresh token should fail validate_access_token
        assert!(config.validate_access_token(&refresh.token).is_err());
    }

    #[test]
    fn test_admin_role_in_token() {
        let config = JwtConfig::new(b"test-secret-key-for-testing");

        let result = config
            .generate_access_token("uuid-456", "admin_user", UserRole::Admin, "ip")
            .unwrap();

        let claims = config.validate_access_token(&result.token).unwrap();
        assert_eq!(claims.role, UserRole::Admin);
    }

    #[test]
    fn test_invalid_token() {
        let config = JwtConfig::new(b"test-secret-key-for-testing");

        let result = config.validate_access_token("invalid-token");
        assert!(result.is_err());
    }

    #[test]
    fn test_wrong_secret() {
        let config1 = JwtConfig::new(b"secret-1");
        let config2 = JwtConfig::new(b"secret-2");

        let result = config1
            .generate_access_token("uuid-123", "alice", UserRole::User, "ip")
            .unwrap();

        let validation = config2.validate_access_token(&result.token);
        assert!(validation.is_err());
    }

    #[test]
    fn test_expired_token() {
        use std::time::{SystemTime, UNIX_EPOCH};

        let secret = b"test-secret";
        let encoding_key = jsonwebtoken::EncodingKey::from_secret(secret);

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // Create claims with exp in the past
        let claims = AccessClaims {
            sub: "uuid-123".to_string(),
            username: "alice".to_string(),
            role: UserRole::User,
            token_type: TokenType::Access,
            iat: now - 100,
            exp: now - 50, // Expired 50 seconds ago
            ipaddr: "ip".to_string(),
        };

        let token = jsonwebtoken::encode(&Header::default(), &claims, &encoding_key).unwrap();

        let config = JwtConfig::new(secret);
        let result = config.validate_access_token(&token);
        assert!(result.is_err());
    }

    #[test]
    fn test_unique_jti_per_refresh_token() {
        let config = JwtConfig::new(b"test-secret-key-for-testing");

        let result1 = config
            .generate_refresh_token("uuid-123", "alice", UserRole::User)
            .unwrap();

        let result2 = config
            .generate_refresh_token("uuid-123", "alice", UserRole::User)
            .unwrap();

        assert_ne!(
            result1.jti, result2.jti,
            "Each refresh token should have a unique jti"
        );
    }
}
