mod attachments;
mod challenge;
mod encryption;
mod login_challenge;
mod passkey;
mod posts;
mod user;

use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};

pub use attachments::{Attachment, AttachmentStore};
pub use challenge::ChallengeStore;
pub use encryption::{EncryptionSettings, EncryptionSettingsStore};
pub use login_challenge::{AuthChallenge, LoginChallengeStore};
pub use passkey::{PasskeyStore, StoredPasskey};
pub use posts::{Post, PostStore, PostSummary};
pub use user::{User, UserRole, UserStore};

#[derive(Clone)]
pub struct Database {
    pool: SqlitePool,
}

impl Database {
    /// Open or create a database at the given path.
    /// Use ":memory:" for an in-memory database.
    pub async fn open(path: &str) -> Result<Self, sqlx::Error> {
        let url = if path == ":memory:" {
            "sqlite::memory:".to_string()
        } else {
            format!("sqlite:{}?mode=rwc", path)
        };

        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect(&url)
            .await?;

        let db = Self { pool };
        db.migrate().await?;
        Ok(db)
    }

    /// Get the current schema version.
    async fn get_version(&self) -> Result<i32, sqlx::Error> {
        let result: Option<(i32,)> = sqlx::query_as("SELECT version FROM schema_version LIMIT 1")
            .fetch_optional(&self.pool)
            .await?;
        Ok(result.map(|r| r.0).unwrap_or(0))
    }

    /// Set the schema version within a transaction.
    async fn set_version(
        tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
        version: i32,
    ) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM schema_version")
            .execute(&mut **tx)
            .await?;
        sqlx::query("INSERT INTO schema_version (version) VALUES (?)")
            .bind(version)
            .execute(&mut **tx)
            .await?;
        Ok(())
    }

    /// Run database migrations.
    async fn migrate(&self) -> Result<(), sqlx::Error> {
        sqlx::query("CREATE TABLE IF NOT EXISTS schema_version (version INTEGER NOT NULL)")
            .execute(&self.pool)
            .await?;

        let version = self.get_version().await?;

        if version < 1 {
            self.migrate_v1().await?;
        }

        if version < 2 {
            self.migrate_v2().await?;
        }

        if version < 3 {
            self.migrate_v3().await?;
        }

        if version < 4 {
            self.migrate_v4().await?;
        }

        Ok(())
    }

    /// Execute a list of queries in a transaction, then set the version.
    async fn run_migration(
        &self,
        version: i32,
        queries: &[&'static str],
    ) -> Result<(), sqlx::Error> {
        let mut tx = self.pool.begin().await?;
        for query in queries {
            sqlx::query(*query).execute(&mut *tx).await?;
        }
        Self::set_version(&mut tx, version).await?;
        tx.commit().await?;
        Ok(())
    }

    async fn migrate_v1(&self) -> Result<(), sqlx::Error> {
        self.run_migration(
            1,
            &[
                // Users table
                "CREATE TABLE users (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    uuid TEXT UNIQUE NOT NULL,
                    username TEXT UNIQUE NOT NULL COLLATE NOCASE,
                    activated INTEGER NOT NULL DEFAULT 0,
                    role TEXT NOT NULL DEFAULT 'user',
                    created_at TEXT NOT NULL DEFAULT (datetime('now'))
                )",
                "CREATE INDEX idx_users_uuid ON users(uuid)",
                "CREATE INDEX idx_users_username ON users(username)",
                "CREATE INDEX idx_users_activated ON users(activated)",
                // Passkeys table
                "CREATE TABLE passkeys (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    credential_id TEXT UNIQUE NOT NULL,
                    user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
                    passkey_json TEXT NOT NULL,
                    created_at TEXT NOT NULL DEFAULT (datetime('now'))
                )",
                "CREATE INDEX idx_passkeys_user_id ON passkeys(user_id)",
                "CREATE INDEX idx_passkeys_credential_id ON passkeys(credential_id)",
                // Registration challenges table
                "CREATE TABLE registration_challenges (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    user_uuid TEXT UNIQUE NOT NULL,
                    challenge_json TEXT NOT NULL,
                    created_at TEXT NOT NULL DEFAULT (datetime('now'))
                )",
                "CREATE INDEX idx_challenges_user_uuid ON registration_challenges(user_uuid)",
                "CREATE INDEX idx_challenges_created_at ON registration_challenges(created_at)",
                // Login challenges table
                "CREATE TABLE login_challenges (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    session_id TEXT UNIQUE NOT NULL,
                    challenge_json TEXT NOT NULL,
                    created_at TEXT NOT NULL DEFAULT (datetime('now'))
                )",
                "CREATE INDEX idx_login_challenges_session_id ON login_challenges(session_id)",
                "CREATE INDEX idx_login_challenges_created_at ON login_challenges(created_at)",
                // Posts table
                "CREATE TABLE posts (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    uuid TEXT UNIQUE NOT NULL,
                    user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
                    title TEXT,
                    title_encrypted INTEGER NOT NULL DEFAULT 0,
                    content TEXT NOT NULL DEFAULT '',
                    content_encrypted INTEGER NOT NULL DEFAULT 0,
                    created_at TEXT NOT NULL DEFAULT (datetime('now')),
                    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
                )",
                "CREATE INDEX idx_posts_uuid ON posts(uuid)",
                "CREATE INDEX idx_posts_user_id ON posts(user_id)",
                "CREATE INDEX idx_posts_updated_at ON posts(updated_at)",
                // User encryption settings table
                "CREATE TABLE user_encryption_settings (
                    user_id INTEGER PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
                    setup_done INTEGER NOT NULL DEFAULT 0,
                    encryption_enabled,
                    prf_salt BLOB,
                    created_at TEXT NOT NULL DEFAULT (datetime('now'))
                )",
            ],
        )
        .await
    }

    async fn migrate_v2(&self) -> Result<(), sqlx::Error> {
        self.run_migration(
            2,
            &[
                // Add encryption_version column (nullable, only set when content_encrypted=true)
                "ALTER TABLE posts ADD COLUMN encryption_version INTEGER",
                // Add iv column for the initialization vector (nullable, base64url encoded)
                "ALTER TABLE posts ADD COLUMN iv TEXT",
                // Add title_iv column for the title initialization vector
                "ALTER TABLE posts ADD COLUMN title_iv TEXT",
            ],
        )
        .await
    }

    async fn migrate_v3(&self) -> Result<(), sqlx::Error> {
        self.run_migration(
            3,
            &[
                // Add position column for drag-and-drop ordering
                // Lower values appear first. NULL means use updated_at order (legacy).
                "ALTER TABLE posts ADD COLUMN position INTEGER",
                // Index for efficient ordering queries
                "CREATE INDEX idx_posts_position ON posts(user_id, position)",
            ],
        )
        .await
    }

    async fn migrate_v4(&self) -> Result<(), sqlx::Error> {
        self.run_migration(
            4,
            &[
                // Attachments table for encrypted images
                "CREATE TABLE attachments (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    uuid TEXT UNIQUE NOT NULL,
                    user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
                    encrypted_image BLOB NOT NULL,
                    encrypted_image_iv TEXT NOT NULL,
                    encrypted_thumbnail BLOB NOT NULL,
                    encrypted_thumbnail_iv TEXT NOT NULL,
                    encryption_version INTEGER NOT NULL,
                    reference_count INTEGER NOT NULL DEFAULT 0,
                    created_at TEXT NOT NULL DEFAULT (datetime('now'))
                )",
                "CREATE INDEX idx_attachments_uuid ON attachments(uuid)",
                "CREATE INDEX idx_attachments_user_id ON attachments(user_id)",
                "CREATE INDEX idx_attachments_ref_count ON attachments(reference_count)",
                // Post attachment references (for tracking which attachments belong to which post)
                "CREATE TABLE post_attachments (
                    post_id INTEGER NOT NULL REFERENCES posts(id) ON DELETE CASCADE,
                    attachment_uuid TEXT NOT NULL,
                    PRIMARY KEY (post_id, attachment_uuid)
                )",
                "CREATE INDEX idx_post_attachments_attachment ON post_attachments(attachment_uuid)",
            ],
        )
        .await
    }

    /// Get the user store.
    pub fn users(&self) -> UserStore {
        UserStore::new(self.pool.clone())
    }

    /// Get the passkey store.
    pub fn passkeys(&self) -> PasskeyStore {
        PasskeyStore::new(self.pool.clone())
    }

    /// Get the challenge store (for registration).
    pub fn challenges(&self) -> ChallengeStore {
        ChallengeStore::new(self.pool.clone())
    }

    /// Get the login challenge store (unified for all login flows).
    pub fn login_challenges(&self) -> LoginChallengeStore {
        LoginChallengeStore::new(self.pool.clone())
    }

    /// Get the underlying connection pool (for tests that need raw SQL access).
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    /// Get the posts store.
    pub fn posts(&self) -> PostStore {
        PostStore::new(self.pool.clone())
    }

    /// Get the encryption settings store.
    pub fn encryption_settings(&self) -> EncryptionSettingsStore {
        EncryptionSettingsStore::new(self.pool.clone())
    }

    /// Get the attachments store.
    pub fn attachments(&self) -> AttachmentStore {
        AttachmentStore::new(self.pool.clone())
    }

    /// Begin a new transaction.
    pub async fn begin(&self) -> Result<sqlx::Transaction<'_, sqlx::Sqlite>, sqlx::Error> {
        self.pool.begin().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_and_get_user() {
        let db = Database::open(":memory:").await.unwrap();

        let id = db.users().create("uuid-123", "alice").await.unwrap();

        let user = db.users().get_by_username("alice").await.unwrap().unwrap();
        assert_eq!(user.id, id);
        assert_eq!(user.uuid, "uuid-123");
        assert_eq!(user.username, "alice");
        assert!(!user.activated);

        let user = db.users().get_by_id(id).await.unwrap().unwrap();
        assert_eq!(user.id, id);

        let user = db.users().get_by_uuid("uuid-123").await.unwrap().unwrap();
        assert_eq!(user.id, id);
    }

    #[tokio::test]
    async fn test_activate_user() {
        let db = Database::open(":memory:").await.unwrap();

        let id = db.users().create("uuid-123", "alice").await.unwrap();
        assert!(!db.users().get_by_id(id).await.unwrap().unwrap().activated);

        db.users().activate(id).await.unwrap();
        assert!(db.users().get_by_id(id).await.unwrap().unwrap().activated);
    }

    #[tokio::test]
    async fn test_duplicate_username_fails() {
        let db = Database::open(":memory:").await.unwrap();

        db.users().create("uuid-1", "alice").await.unwrap();
        let result = db.users().create("uuid-2", "alice").await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_username_availability() {
        let db = Database::open(":memory:").await.unwrap();

        assert!(db.users().is_username_available("alice").await.unwrap());

        db.users().create("uuid-1", "alice").await.unwrap();
        assert!(!db.users().is_username_available("alice").await.unwrap());
    }

    #[tokio::test]
    async fn test_delete_user() {
        let db = Database::open(":memory:").await.unwrap();

        let id = db.users().create("uuid-123", "alice").await.unwrap();
        db.users().delete(id).await.unwrap();

        assert!(db.users().get_by_id(id).await.unwrap().is_none());
    }
}
