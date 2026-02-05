//! Post storage for post entries with hierarchical structure support.

use sqlx::sqlite::SqlitePool;
use std::collections::HashMap;

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
    pub parent_id: Option<String>,
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
    pub parent_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// A post node in the tree structure.
#[derive(Debug, Clone)]
pub struct PostNode {
    pub uuid: String,
    pub title: Option<String>,
    pub title_encrypted: bool,
    pub title_iv: Option<String>,
    pub content_encrypted: bool,
    pub encryption_version: Option<i32>,
    pub position: Option<i32>,
    pub parent_id: Option<String>,
    pub has_children: bool,
    pub children: Option<Vec<PostNode>>,
    pub created_at: String,
    pub updated_at: String,
}

/// Result of a delete operation.
#[derive(Debug, Clone)]
pub struct DeleteResult {
    pub deleted: bool,
    pub children_deleted: i64,
}

/// Parameters for updating a post with optional attachment references.
pub struct UpdatePostParams<'a> {
    pub uuid: &'a str,
    pub user_id: i64,
    pub title: Option<&'a str>,
    pub title_encrypted: bool,
    pub title_iv: Option<&'a str>,
    pub content: &'a str,
    pub content_encrypted: bool,
    pub iv: Option<&'a str>,
    pub encryption_version: Option<i32>,
    pub attachment_uuids: Option<&'a [String]>,
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
    parent_id: Option<String>,
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
            parent_id: row.parent_id,
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
    parent_id: Option<String>,
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
            parent_id: row.parent_id,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

impl From<PostSummary> for PostNode {
    fn from(summary: PostSummary) -> Self {
        Self {
            uuid: summary.uuid,
            title: summary.title,
            title_encrypted: summary.title_encrypted,
            title_iv: summary.title_iv,
            content_encrypted: summary.content_encrypted,
            encryption_version: summary.encryption_version,
            position: summary.position,
            parent_id: summary.parent_id,
            has_children: false,
            children: Some(Vec::new()),
            created_at: summary.created_at,
            updated_at: summary.updated_at,
        }
    }
}

impl PostStore {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Create a new post. Returns the post UUID.
    /// New posts are inserted at position 0 under the specified parent,
    /// shifting all existing siblings down.
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
        parent_id: Option<&str>,
    ) -> Result<String, sqlx::Error> {
        let uuid = uuid::Uuid::new_v4().to_string();

        // Validate parent_id if provided
        if let Some(pid) = parent_id {
            let parent_exists: Option<(i64,)> =
                sqlx::query_as("SELECT 1 FROM posts WHERE uuid = ? AND user_id = ?")
                    .bind(pid)
                    .bind(user_id)
                    .fetch_optional(&self.pool)
                    .await?;
            if parent_exists.is_none() {
                return Err(sqlx::Error::RowNotFound);
            }
        }

        // Start a transaction to atomically shift positions and insert
        let mut tx = self.pool.begin().await?;

        // Shift all existing siblings down by 1
        if parent_id.is_some() {
            sqlx::query(
                "UPDATE posts SET position = COALESCE(position, 0) + 1 WHERE user_id = ? AND parent_id = ?",
            )
            .bind(user_id)
            .bind(parent_id)
            .execute(&mut *tx)
            .await?;
        } else {
            sqlx::query(
                "UPDATE posts SET position = COALESCE(position, 0) + 1 WHERE user_id = ? AND parent_id IS NULL",
            )
            .bind(user_id)
            .execute(&mut *tx)
            .await?;
        }

        // Insert new post at position 0
        sqlx::query(
            "INSERT INTO posts (uuid, user_id, title, title_encrypted, title_iv, content, content_encrypted, iv, encryption_version, position, parent_id)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 0, ?)",
        )
        .bind(&uuid)
        .bind(user_id)
        .bind(title)
        .bind(title_encrypted)
        .bind(title_iv)
        .bind(content)
        .bind(content_encrypted)
        .bind(iv)
        .bind(encryption_version)
        .bind(parent_id)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(uuid)
    }

    /// Get a post by UUID. Only returns the post if it belongs to the given user.
    pub async fn get_by_uuid(&self, uuid: &str, user_id: i64) -> Result<Option<Post>, sqlx::Error> {
        let row: Option<PostRow> = sqlx::query_as(
            "SELECT id, uuid, user_id, title, title_encrypted, title_iv, content, content_encrypted, iv, encryption_version, position, parent_id, created_at, updated_at
             FROM posts WHERE uuid = ? AND user_id = ?",
        )
        .bind(uuid)
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(Post::from))
    }

    /// List all posts for a user as a flat list, ordered by position.
    /// Posts without a position are sorted by updated_at descending after positioned posts.
    pub async fn list_by_user(&self, user_id: i64) -> Result<Vec<PostSummary>, sqlx::Error> {
        let rows: Vec<PostSummaryRow> = sqlx::query_as(
            "SELECT uuid, title, title_encrypted, title_iv, content_encrypted, encryption_version, position, parent_id, created_at, updated_at
             FROM posts WHERE user_id = ?
             ORDER BY position IS NULL, position ASC, updated_at DESC
             LIMIT 10000",
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(PostSummary::from).collect())
    }

    /// List posts as a tree structure up to the specified depth.
    /// Returns root-level posts with children nested.
    pub async fn list_tree(
        &self,
        user_id: i64,
        max_depth: i32,
    ) -> Result<Vec<PostNode>, sqlx::Error> {
        // Fetch all posts for the user
        let all_posts = self.list_by_user(user_id).await?;

        // Build the tree structure
        Ok(Self::build_tree(all_posts, max_depth))
    }

    /// Build a tree structure from a flat list of posts.
    fn build_tree(posts: Vec<PostSummary>, max_depth: i32) -> Vec<PostNode> {
        // Group posts by parent_id
        let mut by_parent: HashMap<Option<String>, Vec<PostSummary>> = HashMap::new();
        for post in posts {
            by_parent
                .entry(post.parent_id.clone())
                .or_default()
                .push(post);
        }

        // Sort each group by position
        for group in by_parent.values_mut() {
            group.sort_by(|a, b| {
                match (a.position, b.position) {
                    (Some(pa), Some(pb)) => pa.cmp(&pb),
                    (Some(_), None) => std::cmp::Ordering::Less,
                    (None, Some(_)) => std::cmp::Ordering::Greater,
                    (None, None) => b.updated_at.cmp(&a.updated_at), // newer first
                }
            });
        }

        // Recursively build tree starting from root (parent_id = None)
        Self::build_subtree(&by_parent, None, 0, max_depth)
    }

    fn build_subtree(
        by_parent: &HashMap<Option<String>, Vec<PostSummary>>,
        parent_id: Option<String>,
        current_depth: i32,
        max_depth: i32,
    ) -> Vec<PostNode> {
        let Some(children) = by_parent.get(&parent_id) else {
            return Vec::new();
        };

        children
            .iter()
            .map(|post| {
                let child_posts = by_parent.get(&Some(post.uuid.clone()));
                let has_children = child_posts.is_some_and(|c| !c.is_empty());

                let children = if current_depth < max_depth {
                    Some(Self::build_subtree(
                        by_parent,
                        Some(post.uuid.clone()),
                        current_depth + 1,
                        max_depth,
                    ))
                } else if has_children {
                    // Beyond max depth but has children - don't load them
                    None
                } else {
                    Some(Vec::new())
                };

                PostNode {
                    uuid: post.uuid.clone(),
                    title: post.title.clone(),
                    title_encrypted: post.title_encrypted,
                    title_iv: post.title_iv.clone(),
                    content_encrypted: post.content_encrypted,
                    encryption_version: post.encryption_version,
                    position: post.position,
                    parent_id: post.parent_id.clone(),
                    has_children,
                    children,
                    created_at: post.created_at.clone(),
                    updated_at: post.updated_at.clone(),
                }
            })
            .collect()
    }

    /// List immediate children of a post.
    pub async fn list_children(
        &self,
        user_id: i64,
        parent_uuid: &str,
    ) -> Result<Vec<PostNode>, sqlx::Error> {
        // Verify parent exists and belongs to user
        let parent_exists: Option<(i64,)> =
            sqlx::query_as("SELECT 1 FROM posts WHERE uuid = ? AND user_id = ?")
                .bind(parent_uuid)
                .bind(user_id)
                .fetch_optional(&self.pool)
                .await?;
        if parent_exists.is_none() {
            return Err(sqlx::Error::RowNotFound);
        }

        let rows: Vec<PostSummaryRow> = sqlx::query_as(
            "SELECT uuid, title, title_encrypted, title_iv, content_encrypted, encryption_version, position, parent_id, created_at, updated_at
             FROM posts WHERE user_id = ? AND parent_id = ?
             ORDER BY position IS NULL, position ASC, updated_at DESC",
        )
        .bind(user_id)
        .bind(parent_uuid)
        .fetch_all(&self.pool)
        .await?;

        // Convert to PostNodes and check for grandchildren
        let mut nodes = Vec::with_capacity(rows.len());
        for row in rows {
            let summary = PostSummary::from(row);
            let has_children: Option<(i64,)> =
                sqlx::query_as("SELECT 1 FROM posts WHERE parent_id = ? LIMIT 1")
                    .bind(&summary.uuid)
                    .fetch_optional(&self.pool)
                    .await?;

            nodes.push(PostNode {
                uuid: summary.uuid,
                title: summary.title,
                title_encrypted: summary.title_encrypted,
                title_iv: summary.title_iv,
                content_encrypted: summary.content_encrypted,
                encryption_version: summary.encryption_version,
                position: summary.position,
                parent_id: summary.parent_id,
                has_children: has_children.is_some(),
                children: if has_children.is_some() {
                    None
                } else {
                    Some(Vec::new())
                },
                created_at: summary.created_at,
                updated_at: summary.updated_at,
            });
        }

        Ok(nodes)
    }

    /// Get the internal post ID by UUID within an existing transaction.
    /// Returns None if post doesn't exist or doesn't belong to user.
    pub async fn get_id_by_uuid_tx(
        tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
        uuid: &str,
        user_id: i64,
    ) -> Result<Option<i64>, sqlx::Error> {
        let row: Option<(i64,)> =
            sqlx::query_as("SELECT id FROM posts WHERE uuid = ? AND user_id = ?")
                .bind(uuid)
                .bind(user_id)
                .fetch_optional(&mut **tx)
                .await?;
        Ok(row.map(|r| r.0))
    }

    /// Update a post within an existing transaction.
    /// Returns true if the post was updated.
    #[allow(clippy::too_many_arguments)]
    pub async fn update_tx(
        tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
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
        .execute(&mut **tx)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Get all descendant post IDs (including the post itself) within a transaction.
    pub async fn get_descendant_ids_tx(
        tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
        uuid: &str,
        user_id: i64,
    ) -> Result<Vec<i64>, sqlx::Error> {
        let rows: Vec<(i64,)> = sqlx::query_as(
            "WITH RECURSIVE descendants AS (
                SELECT id FROM posts WHERE uuid = ? AND user_id = ?
                UNION ALL
                SELECT p.id FROM posts p
                INNER JOIN descendants d ON p.parent_id = (SELECT uuid FROM posts WHERE id = d.id)
                WHERE p.user_id = ?
            )
            SELECT id FROM descendants",
        )
        .bind(uuid)
        .bind(user_id)
        .bind(user_id)
        .fetch_all(&mut **tx)
        .await?;
        Ok(rows.into_iter().map(|r| r.0).collect())
    }

    /// Delete a post within an existing transaction.
    /// Returns true if the post was deleted.
    pub async fn delete_tx(
        tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
        uuid: &str,
        user_id: i64,
    ) -> Result<bool, sqlx::Error> {
        let result = sqlx::query("DELETE FROM posts WHERE uuid = ? AND user_id = ?")
            .bind(uuid)
            .bind(user_id)
            .execute(&mut **tx)
            .await?;
        Ok(result.rows_affected() > 0)
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

    /// Count all descendants of a post (for delete warning).
    pub async fn count_descendants(&self, uuid: &str, user_id: i64) -> Result<i64, sqlx::Error> {
        // Use recursive CTE to count all descendants
        let result: (i64,) = sqlx::query_as(
            "WITH RECURSIVE descendants AS (
                SELECT uuid FROM posts WHERE parent_id = ? AND user_id = ?
                UNION ALL
                SELECT p.uuid FROM posts p
                INNER JOIN descendants d ON p.parent_id = d.uuid
                WHERE p.user_id = ?
            )
            SELECT COUNT(*) FROM descendants",
        )
        .bind(uuid)
        .bind(user_id)
        .bind(user_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(result.0)
    }

    /// Delete a post by UUID. Only deletes if the post belongs to the given user.
    /// All descendants are also deleted recursively.
    /// Returns DeleteResult with deleted status and count of children deleted.
    pub async fn delete(&self, uuid: &str, user_id: i64) -> Result<DeleteResult, sqlx::Error> {
        // Count descendants first
        let children_count = self.count_descendants(uuid, user_id).await?;

        // Delete all descendants first using recursive CTE
        // This handles the case where parent_id doesn't have ON DELETE CASCADE
        sqlx::query(
            "WITH RECURSIVE descendants AS (
                SELECT uuid FROM posts WHERE parent_id = ? AND user_id = ?
                UNION ALL
                SELECT p.uuid FROM posts p
                INNER JOIN descendants d ON p.parent_id = d.uuid
                WHERE p.user_id = ?
            )
            DELETE FROM posts WHERE uuid IN (SELECT uuid FROM descendants)",
        )
        .bind(uuid)
        .bind(user_id)
        .bind(user_id)
        .execute(&self.pool)
        .await?;

        // Now delete the parent post itself
        let result = sqlx::query("DELETE FROM posts WHERE uuid = ? AND user_id = ?")
            .bind(uuid)
            .bind(user_id)
            .execute(&self.pool)
            .await?;

        Ok(DeleteResult {
            deleted: result.rows_affected() > 0,
            children_deleted: if result.rows_affected() > 0 {
                children_count
            } else {
                0
            },
        })
    }

    /// Move a post to a new parent at the specified position.
    /// If new_parent_id is None, moves to root level.
    pub async fn move_post(
        &self,
        uuid: &str,
        user_id: i64,
        new_parent_id: Option<&str>,
        position: i32,
    ) -> Result<bool, sqlx::Error> {
        // Get current post info
        let post = self.get_by_uuid(uuid, user_id).await?;
        let Some(post) = post else {
            return Ok(false);
        };

        // Validate new parent exists (if specified) and is not a descendant
        if let Some(new_pid) = new_parent_id {
            // Check parent exists
            let parent_exists: Option<(i64,)> =
                sqlx::query_as("SELECT 1 FROM posts WHERE uuid = ? AND user_id = ?")
                    .bind(new_pid)
                    .bind(user_id)
                    .fetch_optional(&self.pool)
                    .await?;
            if parent_exists.is_none() {
                return Err(sqlx::Error::RowNotFound);
            }

            // Check not moving to own descendant (would create a cycle)
            let is_descendant: Option<(i64,)> = sqlx::query_as(
                "WITH RECURSIVE descendants AS (
                    SELECT uuid FROM posts WHERE parent_id = ? AND user_id = ?
                    UNION ALL
                    SELECT p.uuid FROM posts p
                    INNER JOIN descendants d ON p.parent_id = d.uuid
                    WHERE p.user_id = ?
                )
                SELECT 1 FROM descendants WHERE uuid = ?",
            )
            .bind(uuid)
            .bind(user_id)
            .bind(user_id)
            .bind(new_pid)
            .fetch_optional(&self.pool)
            .await?;
            if is_descendant.is_some() {
                // Cannot move a post to its own descendant
                return Err(sqlx::Error::Protocol(
                    "Cannot move a post to its own descendant".to_string(),
                ));
            }
        }

        let mut tx = self.pool.begin().await?;

        // Close gap in old parent
        let old_position = post.position.unwrap_or(0);
        if post.parent_id.is_some() {
            sqlx::query(
                "UPDATE posts SET position = position - 1
                 WHERE user_id = ? AND parent_id = ? AND position > ?",
            )
            .bind(user_id)
            .bind(&post.parent_id)
            .bind(old_position)
            .execute(&mut *tx)
            .await?;
        } else {
            sqlx::query(
                "UPDATE posts SET position = position - 1
                 WHERE user_id = ? AND parent_id IS NULL AND position > ?",
            )
            .bind(user_id)
            .bind(old_position)
            .execute(&mut *tx)
            .await?;
        }

        // Make room in new parent
        if new_parent_id.is_some() {
            sqlx::query(
                "UPDATE posts SET position = position + 1
                 WHERE user_id = ? AND parent_id = ? AND position >= ?",
            )
            .bind(user_id)
            .bind(new_parent_id)
            .bind(position)
            .execute(&mut *tx)
            .await?;
        } else {
            sqlx::query(
                "UPDATE posts SET position = position + 1
                 WHERE user_id = ? AND parent_id IS NULL AND position >= ?",
            )
            .bind(user_id)
            .bind(position)
            .execute(&mut *tx)
            .await?;
        }

        // Move the post
        let result = sqlx::query(
            "UPDATE posts SET parent_id = ?, position = ? WHERE uuid = ? AND user_id = ?",
        )
        .bind(new_parent_id)
        .bind(position)
        .bind(uuid)
        .bind(user_id)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(result.rows_affected() > 0)
    }

    /// Reorder posts within a parent by setting their positions based on the provided UUID order.
    /// The first UUID in the list gets position 0, second gets 1, etc.
    /// Only updates posts belonging to the given user and with the specified parent.
    /// Returns the number of posts updated.
    pub async fn reorder(
        &self,
        user_id: i64,
        parent_id: Option<&str>,
        uuids: &[String],
    ) -> Result<usize, sqlx::Error> {
        if uuids.is_empty() {
            return Ok(0);
        }

        let mut tx = self.pool.begin().await?;
        let mut updated = 0;

        for (position, uuid) in uuids.iter().enumerate() {
            let result = if parent_id.is_some() {
                sqlx::query(
                    "UPDATE posts SET position = ? WHERE uuid = ? AND user_id = ? AND parent_id = ?",
                )
                .bind(position as i32)
                .bind(uuid)
                .bind(user_id)
                .bind(parent_id)
                .execute(&mut *tx)
                .await?
            } else {
                sqlx::query(
                    "UPDATE posts SET position = ? WHERE uuid = ? AND user_id = ? AND parent_id IS NULL",
                )
                .bind(position as i32)
                .bind(uuid)
                .bind(user_id)
                .execute(&mut *tx)
                .await?
            };
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
        assert!(post.parent_id.is_none());
    }

    #[tokio::test]
    async fn test_create_nested_post() {
        let db = Database::open(":memory:").await.unwrap();
        let user_id = db.users().create("uuid-1", "alice").await.unwrap();

        // Create parent post
        let parent_uuid = db
            .posts()
            .create(
                user_id,
                Some("Parent Post"),
                false,
                None,
                "Parent content",
                false,
                None,
                None,
                None,
            )
            .await
            .unwrap();

        // Create child post
        let child_uuid = db
            .posts()
            .create(
                user_id,
                Some("Child Post"),
                false,
                None,
                "Child content",
                false,
                None,
                None,
                Some(&parent_uuid),
            )
            .await
            .unwrap();

        let child = db
            .posts()
            .get_by_uuid(&child_uuid, user_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(child.parent_id, Some(parent_uuid.clone()));

        // Verify tree structure
        let tree = db.posts().list_tree(user_id, 3).await.unwrap();
        assert_eq!(tree.len(), 1);
        assert_eq!(tree[0].uuid, parent_uuid);
        assert!(tree[0].has_children);
        assert_eq!(tree[0].children.as_ref().unwrap().len(), 1);
        assert_eq!(tree[0].children.as_ref().unwrap()[0].uuid, child_uuid);
    }

    #[tokio::test]
    async fn test_delete_with_children() {
        let db = Database::open(":memory:").await.unwrap();
        let user_id = db.users().create("uuid-1", "alice").await.unwrap();

        // Create parent
        let parent_uuid = db
            .posts()
            .create(
                user_id,
                Some("Parent"),
                false,
                None,
                "",
                false,
                None,
                None,
                None,
            )
            .await
            .unwrap();

        // Create children
        let _child1 = db
            .posts()
            .create(
                user_id,
                Some("Child 1"),
                false,
                None,
                "",
                false,
                None,
                None,
                Some(&parent_uuid),
            )
            .await
            .unwrap();

        let child2 = db
            .posts()
            .create(
                user_id,
                Some("Child 2"),
                false,
                None,
                "",
                false,
                None,
                None,
                Some(&parent_uuid),
            )
            .await
            .unwrap();

        // Create grandchild
        let _grandchild = db
            .posts()
            .create(
                user_id,
                Some("Grandchild"),
                false,
                None,
                "",
                false,
                None,
                None,
                Some(&child2),
            )
            .await
            .unwrap();

        // Count descendants
        let count = db
            .posts()
            .count_descendants(&parent_uuid, user_id)
            .await
            .unwrap();
        assert_eq!(count, 3);

        // Delete parent (should cascade)
        let result = db.posts().delete(&parent_uuid, user_id).await.unwrap();
        assert!(result.deleted);
        assert_eq!(result.children_deleted, 3);

        // Verify all deleted
        let posts = db.posts().list_by_user(user_id).await.unwrap();
        assert!(posts.is_empty());
    }

    #[tokio::test]
    async fn test_move_post() {
        let db = Database::open(":memory:").await.unwrap();
        let user_id = db.users().create("uuid-1", "alice").await.unwrap();

        // Create two root posts (parents)
        let parent1 = db
            .posts()
            .create(
                user_id,
                Some("Parent 1"),
                false,
                None,
                "",
                false,
                None,
                None,
                None,
            )
            .await
            .unwrap();

        let parent2 = db
            .posts()
            .create(
                user_id,
                Some("Parent 2"),
                false,
                None,
                "",
                false,
                None,
                None,
                None,
            )
            .await
            .unwrap();

        // Create post under parent1
        let post = db
            .posts()
            .create(
                user_id,
                Some("My Post"),
                false,
                None,
                "",
                false,
                None,
                None,
                Some(&parent1),
            )
            .await
            .unwrap();

        // Move post to parent2
        let moved = db
            .posts()
            .move_post(&post, user_id, Some(&parent2), 0)
            .await
            .unwrap();
        assert!(moved);

        // Verify new parent
        let post_data = db
            .posts()
            .get_by_uuid(&post, user_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(post_data.parent_id, Some(parent2.clone()));
    }

    #[tokio::test]
    async fn test_reorder_siblings() {
        let db = Database::open(":memory:").await.unwrap();
        let user_id = db.users().create("uuid-1", "alice").await.unwrap();

        // Create parent
        let parent = db
            .posts()
            .create(
                user_id,
                Some("Parent"),
                false,
                None,
                "",
                false,
                None,
                None,
                None,
            )
            .await
            .unwrap();

        // Create children (created in reverse order due to position 0 insertion)
        let child1 = db
            .posts()
            .create(
                user_id,
                Some("Child 1"),
                false,
                None,
                "",
                false,
                None,
                None,
                Some(&parent),
            )
            .await
            .unwrap();

        let child2 = db
            .posts()
            .create(
                user_id,
                Some("Child 2"),
                false,
                None,
                "",
                false,
                None,
                None,
                Some(&parent),
            )
            .await
            .unwrap();

        let child3 = db
            .posts()
            .create(
                user_id,
                Some("Child 3"),
                false,
                None,
                "",
                false,
                None,
                None,
                Some(&parent),
            )
            .await
            .unwrap();

        // Reorder: 1, 2, 3
        let updated = db
            .posts()
            .reorder(
                user_id,
                Some(&parent),
                &[child1.clone(), child2.clone(), child3.clone()],
            )
            .await
            .unwrap();
        assert_eq!(updated, 3);

        // Verify order
        let children = db.posts().list_children(user_id, &parent).await.unwrap();
        assert_eq!(children[0].uuid, child1);
        assert_eq!(children[1].uuid, child2);
        assert_eq!(children[2].uuid, child3);
    }

    #[tokio::test]
    async fn test_list_children() {
        let db = Database::open(":memory:").await.unwrap();
        let user_id = db.users().create("uuid-1", "alice").await.unwrap();

        // Create parent
        let parent = db
            .posts()
            .create(
                user_id,
                Some("Parent"),
                false,
                None,
                "",
                false,
                None,
                None,
                None,
            )
            .await
            .unwrap();

        // Create child with grandchild
        let child = db
            .posts()
            .create(
                user_id,
                Some("Child"),
                false,
                None,
                "",
                false,
                None,
                None,
                Some(&parent),
            )
            .await
            .unwrap();

        let _grandchild = db
            .posts()
            .create(
                user_id,
                Some("Grandchild"),
                false,
                None,
                "",
                false,
                None,
                None,
                Some(&child),
            )
            .await
            .unwrap();

        // List children of parent
        let children = db.posts().list_children(user_id, &parent).await.unwrap();
        assert_eq!(children.len(), 1);
        assert_eq!(children[0].uuid, child);
        assert!(children[0].has_children);
        assert!(children[0].children.is_none()); // Not loaded
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
                None,
            )
            .await
            .unwrap();

        let posts = db.posts().list_by_user(user_id).await.unwrap();
        assert_eq!(posts.len(), 3);

        // Verify all posts are present
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
                None,
            )
            .await
            .unwrap();

        let result = db.posts().delete(&post_uuid, user_id).await.unwrap();
        assert!(result.deleted);
        assert_eq!(result.children_deleted, 0);

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
        let result = db.posts().delete(&post_uuid, bob_id).await.unwrap();
        assert!(!result.deleted);

        // Alice should still have her post unchanged
        let post = db
            .posts()
            .get_by_uuid(&post_uuid, alice_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(post.title, Some("Alice's Post".to_string()));
    }

    #[tokio::test]
    async fn test_cannot_move_to_descendant() {
        let db = Database::open(":memory:").await.unwrap();
        let user_id = db.users().create("uuid-1", "alice").await.unwrap();

        // Create parent
        let parent = db
            .posts()
            .create(
                user_id,
                Some("Parent"),
                false,
                None,
                "",
                false,
                None,
                None,
                None,
            )
            .await
            .unwrap();

        // Create child
        let child = db
            .posts()
            .create(
                user_id,
                Some("Child"),
                false,
                None,
                "",
                false,
                None,
                None,
                Some(&parent),
            )
            .await
            .unwrap();

        // Try to move parent under child (would create cycle)
        let result = db
            .posts()
            .move_post(&parent, user_id, Some(&child), 0)
            .await;
        assert!(result.is_err());
    }
}
