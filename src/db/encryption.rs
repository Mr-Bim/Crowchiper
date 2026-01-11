//! Encryption settings storage.

use sqlx::sqlite::SqlitePool;

/// User encryption settings.
#[derive(Debug, Clone)]
pub struct EncryptionSettings {
    pub user_id: i64,
    /// Whether encryption setup has been completed
    pub setup_done: bool,
    pub encryption_enabled: bool,
    /// PRF salt (32 bytes) - used as input to PRF for key derivation
    pub prf_salt: Option<Vec<u8>>,
    pub created_at: String,
}

#[derive(sqlx::FromRow)]
struct EncryptionSettingsRow {
    user_id: i64,
    setup_done: i32,
    encryption_enabled: i32,
    prf_salt: Option<Vec<u8>>,
    created_at: String,
}

impl From<EncryptionSettingsRow> for EncryptionSettings {
    fn from(row: EncryptionSettingsRow) -> Self {
        Self {
            user_id: row.user_id,
            setup_done: row.setup_done != 0,
            encryption_enabled: row.encryption_enabled != 0,
            prf_salt: row.prf_salt,
            created_at: row.created_at,
        }
    }
}

/// Store for user encryption settings.
#[derive(Clone)]
pub struct EncryptionSettingsStore {
    pool: SqlitePool,
}

impl EncryptionSettingsStore {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Get encryption settings for a user.
    pub async fn get(&self, user_id: i64) -> Result<Option<EncryptionSettings>, sqlx::Error> {
        let row: Option<EncryptionSettingsRow> = sqlx::query_as(
            "SELECT user_id, setup_done, encryption_enabled, prf_salt, created_at
             FROM user_encryption_settings WHERE user_id = ?",
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(EncryptionSettings::from))
    }

    /// Create encryption settings for a user with setup complete and encryption enabled.
    pub async fn create(&self, user_id: i64, prf_salt: &[u8]) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO user_encryption_settings (user_id, setup_done, encryption_enabled, prf_salt) VALUES (?, 1, 1, ?)",
        )
        .bind(user_id)
        .bind(prf_salt)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Mark setup as done without encryption (PRF not supported).
    pub async fn mark_setup_done(&self, user_id: i64) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO user_encryption_settings (user_id, setup_done, encryption_enabled, prf_salt) VALUES (?, 1, 0, NULL)",
        )
        .bind(user_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Delete encryption settings for a user.
    pub async fn delete(&self, user_id: i64) -> Result<bool, sqlx::Error> {
        let result = sqlx::query("DELETE FROM user_encryption_settings WHERE user_id = ?")
            .bind(user_id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Enable encryption for testing without a PRF salt.
    /// The test will inject the encryption key directly via JavaScript.
    #[cfg(feature = "test-mode")]
    pub async fn enable_for_test(&self, user_id: i64) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO user_encryption_settings (user_id, setup_done, encryption_enabled, prf_salt)
             VALUES (?, 1, 1, NULL)
             ON CONFLICT(user_id) DO UPDATE SET setup_done = 1, encryption_enabled = 1",
        )
        .bind(user_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::db::Database;

    #[tokio::test]
    async fn test_encryption_settings_with_prf() {
        let db = Database::open(":memory:").await.unwrap();
        let user_id = db.users().create("uuid-1", "alice").await.unwrap();

        // Initially no settings
        let settings = db.encryption_settings().get(user_id).await.unwrap();
        assert!(settings.is_none());

        // Create settings with PRF salt
        let prf_salt = vec![1u8; 32];
        db.encryption_settings()
            .create(user_id, &prf_salt)
            .await
            .unwrap();

        // Read back
        let loaded = db
            .encryption_settings()
            .get(user_id)
            .await
            .unwrap()
            .unwrap();
        assert!(loaded.setup_done);
        assert!(loaded.encryption_enabled);
        assert_eq!(loaded.prf_salt, Some(prf_salt));

        // Delete
        let deleted = db.encryption_settings().delete(user_id).await.unwrap();
        assert!(deleted);
        let settings = db.encryption_settings().get(user_id).await.unwrap();
        assert!(settings.is_none());
    }

    #[tokio::test]
    async fn test_encryption_settings_without_prf() {
        let db = Database::open(":memory:").await.unwrap();
        let user_id = db.users().create("uuid-1", "bob").await.unwrap();

        // Mark setup done without PRF
        db.encryption_settings()
            .mark_setup_done(user_id)
            .await
            .unwrap();

        // Read back
        let loaded = db
            .encryption_settings()
            .get(user_id)
            .await
            .unwrap()
            .unwrap();
        assert!(loaded.setup_done);
        assert!(!loaded.encryption_enabled);
        assert!(loaded.prf_salt.is_none());
    }
}
