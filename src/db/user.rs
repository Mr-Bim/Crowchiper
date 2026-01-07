use sqlx::sqlite::SqlitePool;

#[derive(Clone)]
pub struct UserStore {
    pool: SqlitePool,
}

/// User role for authorization.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UserRole {
    User,
    Admin,
}

impl UserRole {
    pub fn as_str(&self) -> &'static str {
        match self {
            UserRole::User => "user",
            UserRole::Admin => "admin",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "admin" => UserRole::Admin,
            _ => UserRole::User,
        }
    }
}

#[derive(Debug, Clone)]
pub struct User {
    pub id: i64,
    pub uuid: String,
    pub username: String,
    pub activated: bool,
    pub role: UserRole,
}

#[derive(sqlx::FromRow)]
struct UserRow {
    id: i64,
    uuid: String,
    username: String,
    activated: i32,
    role: String,
}

impl From<UserRow> for User {
    fn from(row: UserRow) -> Self {
        Self {
            id: row.id,
            uuid: row.uuid,
            username: row.username,
            activated: row.activated != 0,
            role: UserRole::from_str(&row.role),
        }
    }
}

impl UserStore {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Create a new pending user (not yet activated). Returns the user ID.
    pub async fn create(&self, uuid: &str, username: &str) -> Result<i64, sqlx::Error> {
        let result = sqlx::query("INSERT INTO users (uuid, username, activated) VALUES (?, ?, 0)")
            .bind(uuid)
            .bind(username)
            .execute(&self.pool)
            .await?;
        Ok(result.last_insert_rowid())
    }

    /// Create a new pending admin user (not yet activated). Returns the user ID.
    pub async fn create_admin(&self, uuid: &str, username: &str) -> Result<i64, sqlx::Error> {
        let result = sqlx::query(
            "INSERT INTO users (uuid, username, activated, role) VALUES (?, ?, 0, 'admin')",
        )
        .bind(uuid)
        .bind(username)
        .execute(&self.pool)
        .await?;
        Ok(result.last_insert_rowid())
    }

    /// Activate a user (after passkey registration).
    pub async fn activate(&self, id: i64) -> Result<bool, sqlx::Error> {
        let result = sqlx::query("UPDATE users SET activated = 1 WHERE id = ? AND activated = 0")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Get a user by username.
    pub async fn get_by_username(&self, username: &str) -> Result<Option<User>, sqlx::Error> {
        let row: Option<UserRow> = sqlx::query_as(
            "SELECT id, uuid, username, activated, role FROM users WHERE username = ?",
        )
        .bind(username)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(User::from))
    }

    /// Get a user by ID.
    pub async fn get_by_id(&self, id: i64) -> Result<Option<User>, sqlx::Error> {
        let row: Option<UserRow> =
            sqlx::query_as("SELECT id, uuid, username, activated, role FROM users WHERE id = ?")
                .bind(id)
                .fetch_optional(&self.pool)
                .await?;
        Ok(row.map(User::from))
    }

    /// Get a user by UUID.
    pub async fn get_by_uuid(&self, uuid: &str) -> Result<Option<User>, sqlx::Error> {
        let row: Option<UserRow> =
            sqlx::query_as("SELECT id, uuid, username, activated, role FROM users WHERE uuid = ?")
                .bind(uuid)
                .fetch_optional(&self.pool)
                .await?;
        Ok(row.map(User::from))
    }

    /// Set the role for a user.
    pub async fn set_role(&self, id: i64, role: UserRole) -> Result<bool, sqlx::Error> {
        let result = sqlx::query("UPDATE users SET role = ? WHERE id = ?")
            .bind(role.as_str())
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Check if a username is available (not taken by an activated user or recent pending user).
    pub async fn is_username_available(&self, username: &str) -> Result<bool, sqlx::Error> {
        // Clean up old pending users first
        self.cleanup_pending().await?;

        let count: (i32,) = sqlx::query_as("SELECT COUNT(*) FROM users WHERE username = ?")
            .bind(username)
            .fetch_one(&self.pool)
            .await?;
        Ok(count.0 == 0)
    }

    /// Delete pending users older than 5 minutes.
    pub async fn cleanup_pending(&self) -> Result<u64, sqlx::Error> {
        let result = sqlx::query(
            "DELETE FROM users WHERE activated = 0 AND created_at < datetime('now', '-5 minutes')",
        )
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }

    /// Delete a user by ID.
    pub async fn delete(&self, id: i64) -> Result<bool, sqlx::Error> {
        let result = sqlx::query("DELETE FROM users WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Get a pending (not activated) admin user, if one exists.
    pub async fn get_pending_admin(&self) -> Result<Option<User>, sqlx::Error> {
        let row: Option<UserRow> = sqlx::query_as(
            "SELECT id, uuid, username, activated, role FROM users WHERE role = 'admin' AND activated = 0",
        )
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(User::from))
    }
}
