use sqlx::sqlite::SqlitePool;
use webauthn_rs::prelude::PasskeyRegistration;

/// Store for WebAuthn registration challenges.
///
/// Challenges are stored in the database keyed by user UUID (the client-facing
/// identifier). This allows:
/// - Persistence across server restarts
/// - Scaling to multiple server instances
/// - Automatic expiration via cleanup
#[derive(Clone)]
pub struct ChallengeStore {
    pool: SqlitePool,
}

impl ChallengeStore {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Store a registration challenge for a user.
    ///
    /// Replaces any existing challenge for the same user UUID.
    /// The `user_uuid` is the UUID assigned when the user claimed their username,
    /// not the database row ID.
    pub async fn store(
        &self,
        user_uuid: &str,
        challenge: &PasskeyRegistration,
    ) -> Result<(), sqlx::Error> {
        // Clean up expired challenges on every store
        self.cleanup_expired().await?;

        let json =
            serde_json::to_string(challenge).map_err(|e| sqlx::Error::Encode(Box::new(e)))?;

        // Use INSERT OR REPLACE to handle existing challenges for the same user
        sqlx::query(
            "INSERT OR REPLACE INTO registration_challenges (user_uuid, challenge_json, created_at)
             VALUES (?, ?, datetime('now'))",
        )
        .bind(user_uuid)
        .bind(&json)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Get and remove a registration challenge for a user.
    ///
    /// Returns None if no challenge exists or if it has expired (older than 3 minutes).
    /// The challenge is always removed from the database, even if expired.
    pub async fn take(&self, user_uuid: &str) -> Result<Option<PasskeyRegistration>, sqlx::Error> {
        // First, clean up expired challenges
        self.cleanup_expired().await?;

        // Get the challenge
        let row: Option<(String,)> = sqlx::query_as(
            "SELECT challenge_json FROM registration_challenges WHERE user_uuid = ?",
        )
        .bind(user_uuid)
        .fetch_optional(&self.pool)
        .await?;

        // Remove it regardless of whether we found it
        sqlx::query("DELETE FROM registration_challenges WHERE user_uuid = ?")
            .bind(user_uuid)
            .execute(&self.pool)
            .await?;

        match row {
            Some((json,)) => {
                let challenge: PasskeyRegistration =
                    serde_json::from_str(&json).map_err(|e| sqlx::Error::Decode(Box::new(e)))?;
                Ok(Some(challenge))
            }
            None => Ok(None),
        }
    }

    /// Remove expired challenges (older than 3 minutes).
    ///
    /// Called automatically by `store()` and `take()`.
    pub async fn cleanup_expired(&self) -> Result<u64, sqlx::Error> {
        let result = sqlx::query(
            "DELETE FROM registration_challenges WHERE created_at < datetime('now', '-3 minutes')",
        )
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }
}

#[cfg(test)]
mod tests {
    use crate::db::Database;

    // Note: We can't easily test PasskeyRegistration serialization without
    // a real WebAuthn instance, so we test the database operations work
    // at the SQL level in integration tests.

    #[tokio::test]
    async fn test_cleanup_expired() {
        let db = Database::open(":memory:").await.unwrap();

        // Insert an old challenge directly
        sqlx::query(
            "INSERT INTO registration_challenges (user_uuid, challenge_json, created_at)
             VALUES ('old-uuid', '{}', datetime('now', '-10 minutes'))",
        )
        .execute(&db.challenges().pool)
        .await
        .unwrap();

        // Insert a fresh challenge
        sqlx::query(
            "INSERT INTO registration_challenges (user_uuid, challenge_json, created_at)
             VALUES ('new-uuid', '{}', datetime('now'))",
        )
        .execute(&db.challenges().pool)
        .await
        .unwrap();

        // Cleanup should remove the old one
        let removed = db.challenges().cleanup_expired().await.unwrap();
        assert_eq!(removed, 1);

        // Verify old is gone, new remains
        let count: (i32,) = sqlx::query_as(
            "SELECT COUNT(*) FROM registration_challenges WHERE user_uuid = 'old-uuid'",
        )
        .fetch_one(&db.challenges().pool)
        .await
        .unwrap();
        assert_eq!(count.0, 0);

        let count: (i32,) = sqlx::query_as(
            "SELECT COUNT(*) FROM registration_challenges WHERE user_uuid = 'new-uuid'",
        )
        .fetch_one(&db.challenges().pool)
        .await
        .unwrap();
        assert_eq!(count.0, 1);
    }
}
