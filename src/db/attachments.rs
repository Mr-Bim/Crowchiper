//! Attachment storage for encrypted images with reference counting.

use sqlx::sqlite::SqlitePool;

#[derive(Clone)]
pub struct AttachmentStore {
    pool: SqlitePool,
}

/// A full attachment with all data.
#[derive(Debug, Clone)]
pub struct Attachment {
    pub id: i64,
    pub uuid: String,
    pub user_id: i64,
    pub encrypted_image: Vec<u8>,
    pub encrypted_image_iv: String,
    pub encrypted_thumbnail: Vec<u8>,
    pub encrypted_thumbnail_iv: String,
    pub encryption_version: i32,
    pub reference_count: i32,
    pub created_at: String,
}

#[derive(sqlx::FromRow)]
struct AttachmentRow {
    id: i64,
    uuid: String,
    user_id: i64,
    encrypted_image: Vec<u8>,
    encrypted_image_iv: String,
    encrypted_thumbnail: Vec<u8>,
    encrypted_thumbnail_iv: String,
    encryption_version: i32,
    reference_count: i32,
    created_at: String,
}

impl From<AttachmentRow> for Attachment {
    fn from(row: AttachmentRow) -> Self {
        Self {
            id: row.id,
            uuid: row.uuid,
            user_id: row.user_id,
            encrypted_image: row.encrypted_image,
            encrypted_image_iv: row.encrypted_image_iv,
            encrypted_thumbnail: row.encrypted_thumbnail,
            encrypted_thumbnail_iv: row.encrypted_thumbnail_iv,
            encryption_version: row.encryption_version,
            reference_count: row.reference_count,
            created_at: row.created_at,
        }
    }
}

impl AttachmentStore {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Create a new attachment. Returns the attachment UUID.
    /// Reference count starts at 0 (incremented when added to a post).
    #[allow(clippy::too_many_arguments)]
    pub async fn create(
        &self,
        user_id: i64,
        encrypted_image: &[u8],
        encrypted_image_iv: &str,
        encrypted_thumbnail: &[u8],
        encrypted_thumbnail_iv: &str,
        encryption_version: i32,
    ) -> Result<String, sqlx::Error> {
        let uuid = uuid::Uuid::new_v4().to_string();

        sqlx::query(
            "INSERT INTO attachments (uuid, user_id, encrypted_image, encrypted_image_iv,
             encrypted_thumbnail, encrypted_thumbnail_iv, encryption_version, reference_count)
             VALUES (?, ?, ?, ?, ?, ?, ?, 0)",
        )
        .bind(&uuid)
        .bind(user_id)
        .bind(encrypted_image)
        .bind(encrypted_image_iv)
        .bind(encrypted_thumbnail)
        .bind(encrypted_thumbnail_iv)
        .bind(encryption_version)
        .execute(&self.pool)
        .await?;

        Ok(uuid)
    }

    /// Get an attachment by UUID. Only returns if it belongs to the given user.
    pub async fn get_by_uuid(
        &self,
        uuid: &str,
        user_id: i64,
    ) -> Result<Option<Attachment>, sqlx::Error> {
        let row: Option<AttachmentRow> = sqlx::query_as(
            "SELECT id, uuid, user_id, encrypted_image, encrypted_image_iv,
             encrypted_thumbnail, encrypted_thumbnail_iv, encryption_version,
             reference_count, created_at
             FROM attachments WHERE uuid = ? AND user_id = ?",
        )
        .bind(uuid)
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(Attachment::from))
    }

    /// Get just the thumbnail for an attachment.
    pub async fn get_thumbnail(
        &self,
        uuid: &str,
        user_id: i64,
    ) -> Result<Option<(Vec<u8>, String)>, sqlx::Error> {
        let row: Option<(Vec<u8>, String)> = sqlx::query_as(
            "SELECT encrypted_thumbnail, encrypted_thumbnail_iv
             FROM attachments WHERE uuid = ? AND user_id = ?",
        )
        .bind(uuid)
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    /// Increment reference count for an attachment.
    /// Returns true if the attachment exists and was updated.
    pub async fn increment_ref(&self, uuid: &str, user_id: i64) -> Result<bool, sqlx::Error> {
        let result = sqlx::query(
            "UPDATE attachments SET reference_count = reference_count + 1
             WHERE uuid = ? AND user_id = ?",
        )
        .bind(uuid)
        .bind(user_id)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Decrement reference count for an attachment.
    /// If count reaches 0, the attachment is deleted.
    /// Returns true if the attachment was decremented or deleted.
    pub async fn decrement_ref(&self, uuid: &str, user_id: i64) -> Result<bool, sqlx::Error> {
        let mut tx = self.pool.begin().await?;

        // Decrement the reference count
        let result = sqlx::query(
            "UPDATE attachments SET reference_count = reference_count - 1
             WHERE uuid = ? AND user_id = ? AND reference_count > 0",
        )
        .bind(uuid)
        .bind(user_id)
        .execute(&mut *tx)
        .await?;

        if result.rows_affected() == 0 {
            tx.rollback().await?;
            return Ok(false);
        }

        // Delete if reference count is now 0
        sqlx::query(
            "DELETE FROM attachments WHERE uuid = ? AND user_id = ? AND reference_count = 0",
        )
        .bind(uuid)
        .bind(user_id)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(true)
    }

    /// Get all attachment UUIDs for a post.
    pub async fn get_post_attachments(&self, post_id: i64) -> Result<Vec<String>, sqlx::Error> {
        let rows: Vec<(String,)> =
            sqlx::query_as("SELECT attachment_uuid FROM post_attachments WHERE post_id = ?")
                .bind(post_id)
                .fetch_all(&self.pool)
                .await?;
        Ok(rows.into_iter().map(|r| r.0).collect())
    }

    /// Update attachment references for a post.
    /// Computes the diff between current and new attachments, updates ref counts accordingly.
    pub async fn update_post_attachments(
        &self,
        post_id: i64,
        user_id: i64,
        new_uuids: &[String],
    ) -> Result<(), sqlx::Error> {
        let mut tx = self.pool.begin().await?;

        // Get current attachments for this post
        let current_rows: Vec<(String,)> =
            sqlx::query_as("SELECT attachment_uuid FROM post_attachments WHERE post_id = ?")
                .bind(post_id)
                .fetch_all(&mut *tx)
                .await?;
        let current: std::collections::HashSet<String> =
            current_rows.into_iter().map(|r| r.0).collect();

        let new_set: std::collections::HashSet<String> = new_uuids.iter().cloned().collect();

        // Find removed attachments (in current but not in new)
        for uuid in current.difference(&new_set) {
            // Decrement reference count
            sqlx::query(
                "UPDATE attachments SET reference_count = reference_count - 1
                 WHERE uuid = ? AND user_id = ? AND reference_count > 0",
            )
            .bind(uuid)
            .bind(user_id)
            .execute(&mut *tx)
            .await?;

            // Delete if reference count is now 0
            sqlx::query(
                "DELETE FROM attachments WHERE uuid = ? AND user_id = ? AND reference_count = 0",
            )
            .bind(uuid)
            .bind(user_id)
            .execute(&mut *tx)
            .await?;

            // Remove from post_attachments
            sqlx::query("DELETE FROM post_attachments WHERE post_id = ? AND attachment_uuid = ?")
                .bind(post_id)
                .bind(uuid)
                .execute(&mut *tx)
                .await?;
        }

        // Find added attachments (in new but not in current)
        for uuid in new_set.difference(&current) {
            // Increment reference count
            sqlx::query(
                "UPDATE attachments SET reference_count = reference_count + 1
                 WHERE uuid = ? AND user_id = ?",
            )
            .bind(uuid)
            .bind(user_id)
            .execute(&mut *tx)
            .await?;

            // Add to post_attachments
            sqlx::query(
                "INSERT OR IGNORE INTO post_attachments (post_id, attachment_uuid) VALUES (?, ?)",
            )
            .bind(post_id)
            .bind(uuid)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(())
    }

    /// Remove all attachment references for a post (called on post delete).
    /// Decrements ref counts and deletes attachments with 0 refs.
    pub async fn remove_post_attachments(
        &self,
        post_id: i64,
        user_id: i64,
    ) -> Result<(), sqlx::Error> {
        let mut tx = self.pool.begin().await?;

        // Get current attachments for this post
        let current_rows: Vec<(String,)> =
            sqlx::query_as("SELECT attachment_uuid FROM post_attachments WHERE post_id = ?")
                .bind(post_id)
                .fetch_all(&mut *tx)
                .await?;

        // Decrement ref count for each and delete if 0
        for (uuid,) in current_rows {
            sqlx::query(
                "UPDATE attachments SET reference_count = reference_count - 1
                 WHERE uuid = ? AND user_id = ? AND reference_count > 0",
            )
            .bind(&uuid)
            .bind(user_id)
            .execute(&mut *tx)
            .await?;

            sqlx::query(
                "DELETE FROM attachments WHERE uuid = ? AND user_id = ? AND reference_count = 0",
            )
            .bind(&uuid)
            .bind(user_id)
            .execute(&mut *tx)
            .await?;
        }

        // Remove all post_attachments entries for this post
        sqlx::query("DELETE FROM post_attachments WHERE post_id = ?")
            .bind(post_id)
            .execute(&mut *tx)
            .await?;

        tx.commit().await?;
        Ok(())
    }

    /// Delete an attachment directly by UUID (bypasses reference counting).
    /// Used for cleanup of failed uploads.
    pub async fn delete(&self, uuid: &str, user_id: i64) -> Result<bool, sqlx::Error> {
        let result = sqlx::query("DELETE FROM attachments WHERE uuid = ? AND user_id = ?")
            .bind(uuid)
            .bind(user_id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }
}

#[cfg(test)]
mod tests {
    use crate::db::Database;

    #[tokio::test]
    async fn test_create_and_get_attachment() {
        let db = Database::open(":memory:").await.unwrap();
        let user_id = db.users().create("uuid-1", "alice").await.unwrap();

        let attachment_uuid = db
            .attachments()
            .create(
                user_id,
                b"encrypted_image_data",
                "image_iv_123",
                b"encrypted_thumb_data",
                "thumb_iv_456",
                1,
            )
            .await
            .unwrap();

        let attachment = db
            .attachments()
            .get_by_uuid(&attachment_uuid, user_id)
            .await
            .unwrap()
            .unwrap();

        assert_eq!(attachment.uuid, attachment_uuid);
        assert_eq!(attachment.user_id, user_id);
        assert_eq!(attachment.encrypted_image, b"encrypted_image_data");
        assert_eq!(attachment.encrypted_image_iv, "image_iv_123");
        assert_eq!(attachment.encrypted_thumbnail, b"encrypted_thumb_data");
        assert_eq!(attachment.encrypted_thumbnail_iv, "thumb_iv_456");
        assert_eq!(attachment.encryption_version, 1);
        assert_eq!(attachment.reference_count, 0);
    }

    #[tokio::test]
    async fn test_get_thumbnail() {
        let db = Database::open(":memory:").await.unwrap();
        let user_id = db.users().create("uuid-1", "alice").await.unwrap();

        let attachment_uuid = db
            .attachments()
            .create(
                user_id,
                b"encrypted_image_data",
                "image_iv_123",
                b"encrypted_thumb_data",
                "thumb_iv_456",
                1,
            )
            .await
            .unwrap();

        let (thumb, iv) = db
            .attachments()
            .get_thumbnail(&attachment_uuid, user_id)
            .await
            .unwrap()
            .unwrap();

        assert_eq!(thumb, b"encrypted_thumb_data");
        assert_eq!(iv, "thumb_iv_456");
    }

    #[tokio::test]
    async fn test_reference_counting() {
        let db = Database::open(":memory:").await.unwrap();
        let user_id = db.users().create("uuid-1", "alice").await.unwrap();

        let uuid = db
            .attachments()
            .create(user_id, b"img", "iv1", b"thumb", "iv2", 1)
            .await
            .unwrap();

        // Increment ref count
        assert!(
            db.attachments()
                .increment_ref(&uuid, user_id)
                .await
                .unwrap()
        );
        let attachment = db
            .attachments()
            .get_by_uuid(&uuid, user_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(attachment.reference_count, 1);

        // Increment again
        assert!(
            db.attachments()
                .increment_ref(&uuid, user_id)
                .await
                .unwrap()
        );
        let attachment = db
            .attachments()
            .get_by_uuid(&uuid, user_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(attachment.reference_count, 2);

        // Decrement
        assert!(
            db.attachments()
                .decrement_ref(&uuid, user_id)
                .await
                .unwrap()
        );
        let attachment = db
            .attachments()
            .get_by_uuid(&uuid, user_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(attachment.reference_count, 1);

        // Decrement to 0 - should delete
        assert!(
            db.attachments()
                .decrement_ref(&uuid, user_id)
                .await
                .unwrap()
        );
        let attachment = db.attachments().get_by_uuid(&uuid, user_id).await.unwrap();
        assert!(attachment.is_none());
    }

    #[tokio::test]
    async fn test_cannot_access_other_users_attachment() {
        let db = Database::open(":memory:").await.unwrap();
        let alice_id = db.users().create("uuid-1", "alice").await.unwrap();
        let bob_id = db.users().create("uuid-2", "bob").await.unwrap();

        let uuid = db
            .attachments()
            .create(alice_id, b"img", "iv1", b"thumb", "iv2", 1)
            .await
            .unwrap();

        // Bob should not be able to get Alice's attachment
        let attachment = db.attachments().get_by_uuid(&uuid, bob_id).await.unwrap();
        assert!(attachment.is_none());

        // Bob should not be able to increment Alice's attachment refs
        assert!(!db.attachments().increment_ref(&uuid, bob_id).await.unwrap());

        // Bob should not be able to delete Alice's attachment
        assert!(!db.attachments().delete(&uuid, bob_id).await.unwrap());
    }

    #[tokio::test]
    async fn test_post_attachments() {
        let db = Database::open(":memory:").await.unwrap();
        let user_id = db.users().create("uuid-1", "alice").await.unwrap();

        // Create a post
        let post_uuid = db
            .posts()
            .create(
                user_id,
                Some("Test"),
                false,
                None,
                "content",
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

        // Create two attachments
        let att1_uuid = db
            .attachments()
            .create(user_id, b"img1", "iv1", b"thumb1", "iv1t", 1)
            .await
            .unwrap();
        let att2_uuid = db
            .attachments()
            .create(user_id, b"img2", "iv2", b"thumb2", "iv2t", 1)
            .await
            .unwrap();

        // Add both attachments to the post
        db.attachments()
            .update_post_attachments(post.id, user_id, &[att1_uuid.clone(), att2_uuid.clone()])
            .await
            .unwrap();

        // Check ref counts
        let att1 = db
            .attachments()
            .get_by_uuid(&att1_uuid, user_id)
            .await
            .unwrap()
            .unwrap();
        let att2 = db
            .attachments()
            .get_by_uuid(&att2_uuid, user_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(att1.reference_count, 1);
        assert_eq!(att2.reference_count, 1);

        // Check post_attachments
        let post_atts = db
            .attachments()
            .get_post_attachments(post.id)
            .await
            .unwrap();
        assert_eq!(post_atts.len(), 2);
        assert!(post_atts.contains(&att1_uuid));
        assert!(post_atts.contains(&att2_uuid));

        // Remove att1 from the post
        db.attachments()
            .update_post_attachments(post.id, user_id, &[att2_uuid.clone()])
            .await
            .unwrap();

        // att1 should be deleted (ref count went to 0)
        let att1 = db
            .attachments()
            .get_by_uuid(&att1_uuid, user_id)
            .await
            .unwrap();
        assert!(att1.is_none());

        // att2 should still exist
        let att2 = db
            .attachments()
            .get_by_uuid(&att2_uuid, user_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(att2.reference_count, 1);
    }

    #[tokio::test]
    async fn test_remove_post_attachments_on_delete() {
        let db = Database::open(":memory:").await.unwrap();
        let user_id = db.users().create("uuid-1", "alice").await.unwrap();

        // Create a post
        let post_uuid = db
            .posts()
            .create(
                user_id,
                Some("Test"),
                false,
                None,
                "content",
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

        // Create an attachment
        let att_uuid = db
            .attachments()
            .create(user_id, b"img", "iv", b"thumb", "ivt", 1)
            .await
            .unwrap();

        // Add attachment to post
        db.attachments()
            .update_post_attachments(post.id, user_id, &[att_uuid.clone()])
            .await
            .unwrap();

        // Remove all post attachments (simulating post deletion)
        db.attachments()
            .remove_post_attachments(post.id, user_id)
            .await
            .unwrap();

        // Attachment should be deleted
        let att = db
            .attachments()
            .get_by_uuid(&att_uuid, user_id)
            .await
            .unwrap();
        assert!(att.is_none());
    }
}
