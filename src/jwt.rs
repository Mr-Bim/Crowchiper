//! JWT token generation and validation.

use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::db::UserRole;

/// JWT claims for authentication tokens.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    /// Subject (user UUID)
    pub sub: String,
    /// Username
    pub username: String,
    /// User role
    pub role: UserRole,
    /// Issued at (Unix timestamp)
    pub iat: u64,
    /// Expiration time (Unix timestamp)
    pub exp: u64,
}

/// Configuration for JWT operations.
#[derive(Clone)]
pub struct JwtConfig {
    encoding_key: EncodingKey,
    decoding_key: DecodingKey,
    /// Token validity duration in seconds (default: 24 hours)
    pub token_duration_secs: u64,
}

impl JwtConfig {
    /// Create a new JWT configuration with the given secret.
    pub fn new(secret: &[u8]) -> Self {
        Self {
            encoding_key: EncodingKey::from_secret(secret),
            decoding_key: DecodingKey::from_secret(secret),
            token_duration_secs: 24 * 60 * 60, // 24 hours
        }
    }

    /// Create a new JWT configuration with a custom duration.
    pub fn with_duration(secret: &[u8], duration_secs: u64) -> Self {
        Self {
            encoding_key: EncodingKey::from_secret(secret),
            decoding_key: DecodingKey::from_secret(secret),
            token_duration_secs: duration_secs,
        }
    }

    /// Generate a JWT token for a user.
    pub fn generate_token(
        &self,
        user_uuid: &str,
        username: &str,
        role: UserRole,
    ) -> Result<String, JwtError> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| JwtError::TimeError)?
            .as_secs();

        let claims = Claims {
            sub: user_uuid.to_string(),
            username: username.to_string(),
            role,
            iat: now,
            exp: now + self.token_duration_secs,
        };

        jsonwebtoken::encode(&Header::default(), &claims, &self.encoding_key)
            .map_err(JwtError::Encoding)
    }

    /// Validate and decode a JWT token.
    pub fn validate_token(&self, token: &str) -> Result<Claims, JwtError> {
        let mut validation = Validation::new(Algorithm::HS256);
        validation.leeway = 0; // No grace period for expiration

        let token_data = jsonwebtoken::decode::<Claims>(token, &self.decoding_key, &validation)
            .map_err(JwtError::Decoding)?;

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
}

impl std::fmt::Display for JwtError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            JwtError::Encoding(e) => write!(f, "Failed to encode token: {}", e),
            JwtError::Decoding(e) => write!(f, "Failed to decode token: {}", e),
            JwtError::TimeError => write!(f, "System time error"),
        }
    }
}

impl std::error::Error for JwtError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_and_validate_token() {
        let config = JwtConfig::new(b"test-secret-key-for-testing");

        let token = config
            .generate_token("uuid-123", "alice", UserRole::User)
            .unwrap();

        let claims = config.validate_token(&token).unwrap();
        assert_eq!(claims.sub, "uuid-123");
        assert_eq!(claims.username, "alice");
        assert_eq!(claims.role, UserRole::User);
    }

    #[test]
    fn test_admin_role_in_token() {
        let config = JwtConfig::new(b"test-secret-key-for-testing");

        let token = config
            .generate_token("uuid-456", "admin_user", UserRole::Admin)
            .unwrap();

        let claims = config.validate_token(&token).unwrap();
        assert_eq!(claims.role, UserRole::Admin);
    }

    #[test]
    fn test_invalid_token() {
        let config = JwtConfig::new(b"test-secret-key-for-testing");

        let result = config.validate_token("invalid-token");
        assert!(result.is_err());
    }

    #[test]
    fn test_wrong_secret() {
        let config1 = JwtConfig::new(b"secret-1");
        let config2 = JwtConfig::new(b"secret-2");

        let token = config1
            .generate_token("uuid-123", "alice", UserRole::User)
            .unwrap();

        let result = config2.validate_token(&token);
        assert!(result.is_err());
    }

    #[test]
    fn test_expired_token() {
        // Create a token that's already expired by manipulating claims directly
        use std::time::{SystemTime, UNIX_EPOCH};

        let secret = b"test-secret";
        let encoding_key = jsonwebtoken::EncodingKey::from_secret(secret);

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // Create claims with exp in the past
        let claims = Claims {
            sub: "uuid-123".to_string(),
            username: "alice".to_string(),
            role: UserRole::User,
            iat: now - 100,
            exp: now - 50, // Expired 50 seconds ago
        };

        let token = jsonwebtoken::encode(&Header::default(), &claims, &encoding_key).unwrap();

        let config = JwtConfig::new(secret);
        let result = config.validate_token(&token);
        assert!(result.is_err());
    }
}
