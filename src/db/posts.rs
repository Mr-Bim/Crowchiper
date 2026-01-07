//! Post storage for post entries.

use sqlx::sqlite::SqlitePool;

#[derive(Clone)]
pub struct PostStore {
    pool: SqlitePool,
}

/// A full post with all content.
#[derive(Debug, Clone)]
pub struct Post {
    pub id: i64,
    pub uuid: String,
    pub user_id: i64,
    pub title: Option<String>,
    pub title_encrypted: bool,
    pub title_iv: Option<String>,
    pub content: String,
    pub content_encrypted: bool,
    pub iv: Option<String>,
    pub encryption_version: Option<i32>,
    pub position: Option<i32>,
    pub created_at: String,
    pub updated_at: String,
}

/// A summary of a post for listing (without full content).
#[derive(Debug, Clone)]
pub struct PostSummary {
    pub uuid: String,
    pub title: Option<String>,
    pub title_encrypted: bool,
    pub title_iv: Option<String>,
    pub content_encrypted: bool,
    pub encryption_version: Option<i32>,
    pub position: Option<i32>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(sqlx::FromRow)]
struct PostRow {
    id: i64,
    uuid: String,
    user_id: i64,
    title: Option<String>,
    title_encrypted: bool,
    title_iv: Option<String>,
    content: String,
    content_encrypted: bool,
    iv: Option<String>,
    encryption_version: Option<i32>,
    position: Option<i32>,
    created_at: String,
    updated_at: String,
}

impl From<PostRow> for Post {
    fn from(row: PostRow) -> Self {
        Self {
            id: row.id,
            uuid: row.uuid,
            user_id: row.user_id,
            title: row.title,
            title_encrypted: row.title_encrypted,
            title_iv: row.title_iv,
            content: row.content,
            content_encrypted: row.content_encrypted,
            iv: row.iv,
            encryption_version: row.encryption_version,
            position: row.position,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

#[derive(sqlx::FromRow)]
struct PostSummaryRow {
    uuid: String,
    title: Option<String>,
    title_encrypted: bool,
    title_iv: Option<String>,
    content_encrypted: bool,
    encryption_version: Option<i32>,
    position: Option<i32>,
    created_at: String,
    updated_at: String,
}

impl From<PostSummaryRow> for PostSummary {
    fn from(row: PostSummaryRow) -> Self {
        Self {
            uuid: row.uuid,
            title: row.title,
            title_encrypted: row.title_encrypted,
            title_iv: row.title_iv,
            content_encrypted: row.content_encrypted,
            encryption_version: row.encryption_version,
            position: row.position,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

impl PostStore {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Create a new post. Returns the post UUID.
    /// New posts are inserted at position 0, shifting all existing posts down.
    #[allow(clippy::too_many_arguments)]
    pub async fn create(
        &self,
        user_id: i64,
        title: Option<&str>,
        title_encrypted: bool,
        title_iv: Option<&str>,
        content: &str,
        content_encrypted: bool,
        iv: Option<&str>,
        encryption_version: Option<i32>,
    ) -> Result<String, sqlx::Error> {
        let uuid = uuid::Uuid::new_v4().to_string();

        // Start a transaction to atomically shift positions and insert
        let mut tx = self.pool.begin().await?;

        // Shift all existing posts down by 1
        sqlx::query("UPDATE posts SET position = COALESCE(position, 0) + 1 WHERE user_id = ?")
            .bind(user_id)
            .execute(&mut *tx)
            .await?;

        // Insert new post at position 0
        sqlx::query("INSERT INTO posts (uuid, user_id, title, title_encrypted, title_iv, content, content_encrypted, iv, encryption_version, position) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 0)")
            .bind(&uuid)
            .bind(user_id)
            .bind(title)
            .bind(title_encrypted)
            .bind(title_iv)
            .bind(content)
            .bind(content_encrypted)
            .bind(iv)
            .bind(encryption_version)
            .execute(&mut *tx)
            .await?;

        tx.commit().await?;
        Ok(uuid)
    }

    /// Get a post by UUID. Only returns the post if it belongs to the given user.
    pub async fn get_by_uuid(&self, uuid: &str, user_id: i64) -> Result<Option<Post>, sqlx::Error> {
        let row: Option<PostRow> = sqlx::query_as(
            "SELECT id, uuid, user_id, title, title_encrypted, title_iv, content, content_encrypted, iv, encryption_version, position, created_at, updated_at
             FROM posts WHERE uuid = ? AND user_id = ?",
        )
        .bind(uuid)
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(Post::from))
    }

    /// List all posts for a user, ordered by position (ascending).
    /// Posts without a position are sorted by updated_at descending after positioned posts.
    pub async fn list_by_user(&self, user_id: i64) -> Result<Vec<PostSummary>, sqlx::Error> {
        let rows: Vec<PostSummaryRow> = sqlx::query_as(
            "SELECT uuid, title, title_encrypted, title_iv, content_encrypted, encryption_version, position, created_at, updated_at
             FROM posts WHERE user_id = ?
             ORDER BY position IS NULL, position ASC, updated_at DESC",
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(PostSummary::from).collect())
    }

    /// Update a post by UUID. Only updates if the post belongs to the given user.
    /// Returns true if the post was updated.
    #[allow(clippy::too_many_arguments)]
    pub async fn update(
        &self,
        uuid: &str,
        user_id: i64,
        title: Option<&str>,
        title_encrypted: bool,
        title_iv: Option<&str>,
        content: &str,
        content_encrypted: bool,
        iv: Option<&str>,
        encryption_version: Option<i32>,
    ) -> Result<bool, sqlx::Error> {
        let result = sqlx::query(
            "UPDATE posts SET title = ?, title_encrypted = ?, title_iv = ?, content = ?, content_encrypted = ?, iv = ?, encryption_version = ?, updated_at = datetime('now')
             WHERE uuid = ? AND user_id = ?",
        )
        .bind(title)
        .bind(title_encrypted)
        .bind(title_iv)
        .bind(content)
        .bind(content_encrypted)
        .bind(iv)
        .bind(encryption_version)
        .bind(uuid)
        .bind(user_id)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Delete a post by UUID. Only deletes if the post belongs to the given user.
    /// Returns true if the post was deleted.
    pub async fn delete(&self, uuid: &str, user_id: i64) -> Result<bool, sqlx::Error> {
        let result = sqlx::query("DELETE FROM posts WHERE uuid = ? AND user_id = ?")
            .bind(uuid)
            .bind(user_id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Reorder posts by setting their positions based on the provided UUID order.
    /// The first UUID in the list gets position 0, second gets 1, etc.
    /// Only updates posts belonging to the given user.
    /// Returns the number of posts updated.
    pub async fn reorder(&self, user_id: i64, uuids: &[String]) -> Result<usize, sqlx::Error> {
        if uuids.is_empty() {
            return Ok(0);
        }

        // Use a transaction for atomicity. While this executes multiple statements,
        // SQLite batches them efficiently within a single transaction.
        let mut tx = self.pool.begin().await?;
        let mut updated = 0;

        for (position, uuid) in uuids.iter().enumerate() {
            let result =
                sqlx::query("UPDATE posts SET position = ? WHERE uuid = ? AND user_id = ?")
                    .bind(position as i32)
                    .bind(uuid)
                    .bind(user_id)
                    .execute(&mut *tx)
                    .await?;
            updated += result.rows_affected() as usize;
        }

        tx.commit().await?;
        Ok(updated)
    }
}

#[cfg(test)]
mod tests {
    use crate::db::Database;

    #[tokio::test]
    async fn test_create_and_get_post() {
        let db = Database::open(":memory:").await.unwrap();
        let user_id = db.users().create("uuid-1", "alice").await.unwrap();

        let post_uuid = db
            .posts()
            .create(
                user_id,
                Some("My First Post"),
                false,
                None,
                "Hello, world!",
                false,
                None,
                None,
            )
            .await
            .unwrap();

        let post = db
            .posts()
            .get_by_uuid(&post_uuid, user_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(post.uuid, post_uuid);
        assert_eq!(post.user_id, user_id);
        assert_eq!(post.title, Some("My First Post".to_string()));
        assert!(!post.title_encrypted);
        assert_eq!(post.content, "Hello, world!");
        assert!(!post.content_encrypted);
    }

    #[tokio::test]
    async fn test_post_without_title() {
        let db = Database::open(":memory:").await.unwrap();
        let user_id = db.users().create("uuid-1", "alice").await.unwrap();

        let post_uuid = db
            .posts()
            .create(
                user_id,
                None,
                false,
                None,
                "Just some content",
                false,
                None,
                None,
            )
            .await
            .unwrap();

        let post = db
            .posts()
            .get_by_uuid(&post_uuid, user_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(post.title, None);
        assert_eq!(post.content, "Just some content");
    }

    #[tokio::test]
    async fn test_list_posts() {
        let db = Database::open(":memory:").await.unwrap();
        let user_id = db.users().create("uuid-1", "alice").await.unwrap();

        db.posts()
            .create(
                user_id,
                Some("Post 1"),
                false,
                None,
                "Content 1",
                false,
                None,
                None,
            )
            .await
            .unwrap();
        db.posts()
            .create(
                user_id,
                Some("Post 2"),
                false,
                None,
                "Content 2",
                false,
                None,
                None,
            )
            .await
            .unwrap();
        db.posts()
            .create(
                user_id,
                Some("Post 3"),
                false,
                None,
                "Content 3",
                false,
                None,
                None,
            )
            .await
            .unwrap();

        let posts = db.posts().list_by_user(user_id).await.unwrap();
        assert_eq!(posts.len(), 3);

        // Verify all posts are present (order is by updated_at DESC, but with same-second
        // timestamps the order may vary)
        let titles: Vec<_> = posts.iter().map(|p| p.title.as_deref()).collect();
        assert!(titles.contains(&Some("Post 1")));
        assert!(titles.contains(&Some("Post 2")));
        assert!(titles.contains(&Some("Post 3")));
    }

    #[tokio::test]
    async fn test_update_post() {
        let db = Database::open(":memory:").await.unwrap();
        let user_id = db.users().create("uuid-1", "alice").await.unwrap();

        let post_uuid = db
            .posts()
            .create(
                user_id,
                Some("Original"),
                false,
                None,
                "Original content",
                false,
                None,
                None,
            )
            .await
            .unwrap();

        let updated = db
            .posts()
            .update(
                &post_uuid,
                user_id,
                Some("Updated"),
                false,
                None,
                "Updated content",
                false,
                None,
                None,
            )
            .await
            .unwrap();
        assert!(updated);

        let post = db
            .posts()
            .get_by_uuid(&post_uuid, user_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(post.title, Some("Updated".to_string()));
        assert_eq!(post.content, "Updated content");
    }

    #[tokio::test]
    async fn test_delete_post() {
        let db = Database::open(":memory:").await.unwrap();
        let user_id = db.users().create("uuid-1", "alice").await.unwrap();

        let post_uuid = db
            .posts()
            .create(
                user_id,
                Some("To Delete"),
                false,
                None,
                "Content",
                false,
                None,
                None,
            )
            .await
            .unwrap();

        let deleted = db.posts().delete(&post_uuid, user_id).await.unwrap();
        assert!(deleted);

        let post = db.posts().get_by_uuid(&post_uuid, user_id).await.unwrap();
        assert!(post.is_none());
    }

    #[tokio::test]
    async fn test_cannot_access_other_users_post() {
        let db = Database::open(":memory:").await.unwrap();
        let alice_id = db.users().create("uuid-1", "alice").await.unwrap();
        let bob_id = db.users().create("uuid-2", "bob").await.unwrap();

        let post_uuid = db
            .posts()
            .create(
                alice_id,
                Some("Alice's Post"),
                false,
                None,
                "Secret content",
                false,
                None,
                None,
            )
            .await
            .unwrap();

        // Bob should not be able to get Alice's post
        let post = db.posts().get_by_uuid(&post_uuid, bob_id).await.unwrap();
        assert!(post.is_none());

        // Bob should not be able to update Alice's post
        let updated = db
            .posts()
            .update(
                &post_uuid,
                bob_id,
                Some("Hacked"),
                false,
                None,
                "Hacked content",
                false,
                None,
                None,
            )
            .await
            .unwrap();
        assert!(!updated);

        // Bob should not be able to delete Alice's post
        let deleted = db.posts().delete(&post_uuid, bob_id).await.unwrap();
        assert!(!deleted);

        // Alice should still have her post unchanged
        let post = db
            .posts()
            .get_by_uuid(&post_uuid, alice_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(post.title, Some("Alice's Post".to_string()));
    }
}
