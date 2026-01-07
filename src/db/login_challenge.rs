use serde::{Deserialize, Serialize};
use sqlx::sqlite::SqlitePool;
use webauthn_rs::prelude::{DiscoverableAuthentication, PasskeyAuthentication};

/// The type of authentication challenge stored.
#[derive(Serialize, Deserialize)]
pub enum AuthChallenge {
    /// Username-based authentication (user's passkeys are pre-selected).
    Passkey(PasskeyAuthentication),
    /// Discoverable authentication (browser shows all passkeys for RP).
    Discoverable(DiscoverableAuthentication),
}

/// Store for WebAuthn login challenges.
///
/// Challenges are stored keyed by a random session ID.
/// This handles both username-based and discoverable authentication flows.
#[derive(Clone)]
pub struct LoginChallengeStore {
    pool: SqlitePool,
}

impl LoginChallengeStore {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Store a login challenge.
    pub async fn store(
        &self,
        session_id: &str,
        challenge: &AuthChallenge,
    ) -> Result<(), sqlx::Error> {
        // Clean up expired challenges on every store
        self.cleanup_expired().await?;

        let json =
            serde_json::to_string(challenge).map_err(|e| sqlx::Error::Encode(Box::new(e)))?;

        sqlx::query(
            "INSERT OR REPLACE INTO login_challenges (session_id, challenge_json, created_at)
             VALUES (?, ?, datetime('now'))",
        )
        .bind(session_id)
        .bind(&json)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Get and remove a login challenge.
    ///
    /// Returns None if no challenge exists or if it has expired (older than 3 minutes).
    pub async fn take(&self, session_id: &str) -> Result<Option<AuthChallenge>, sqlx::Error> {
        // First, clean up expired challenges
        self.cleanup_expired().await?;

        // Get the challenge
        let row: Option<(String,)> =
            sqlx::query_as("SELECT challenge_json FROM login_challenges WHERE session_id = ?")
                .bind(session_id)
                .fetch_optional(&self.pool)
                .await?;

        // Remove it regardless of whether we found it
        sqlx::query("DELETE FROM login_challenges WHERE session_id = ?")
            .bind(session_id)
            .execute(&self.pool)
            .await?;

        match row {
            Some((json,)) => {
                let challenge: AuthChallenge =
                    serde_json::from_str(&json).map_err(|e| sqlx::Error::Decode(Box::new(e)))?;
                Ok(Some(challenge))
            }
            None => Ok(None),
        }
    }

    /// Remove expired challenges (older than 3 minutes).
    pub async fn cleanup_expired(&self) -> Result<u64, sqlx::Error> {
        let result = sqlx::query(
            "DELETE FROM login_challenges WHERE created_at < datetime('now', '-3 minutes')",
        )
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }

    /// Delete a challenge by session ID (used when user aborts login).
    pub async fn delete(&self, session_id: &str) -> Result<bool, sqlx::Error> {
        let result = sqlx::query("DELETE FROM login_challenges WHERE session_id = ?")
            .bind(session_id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }
}
