//! Attachment storage for encrypted images with reference counting.

use sqlx::sqlite::SqlitePool;

#[derive(Clone)]
pub struct AttachmentStore {
    pool: SqlitePool,
}

/// Thumbnail data for a single size.
#[derive(Debug, Clone)]
pub struct ThumbnailData {
    pub data: Vec<u8>,
    /// IV for decryption, None if unencrypted
    pub iv: Option<String>,
}

/// All thumbnail sizes for an attachment.
#[derive(Debug, Clone)]
pub struct Thumbnails {
    pub sm: ThumbnailData,         // 200px
    pub md: Option<ThumbnailData>, // 400px (optional for legacy)
    pub lg: Option<ThumbnailData>, // 800px (optional for legacy)
}

/// A full attachment with all data.
#[derive(Debug, Clone)]
pub struct Attachment {
    pub id: i64,
    pub uuid: String,
    pub user_id: i64,
    pub image_data: Vec<u8>,
    /// IV for decryption, None if unencrypted (encryption_version = 0)
    pub image_iv: Option<String>,
    pub thumbnails: Thumbnails,
    pub encryption_version: i32,
    pub reference_count: i32,
    pub created_at: String,
}

#[derive(sqlx::FromRow)]
struct AttachmentRow {
    id: i64,
    uuid: String,
    user_id: i64,
    image_data: Vec<u8>,
    image_iv: Option<String>,
    thumb_sm: Vec<u8>,
    thumb_sm_iv: Option<String>,
    thumb_md: Option<Vec<u8>>,
    thumb_md_iv: Option<String>,
    thumb_lg: Option<Vec<u8>>,
    thumb_lg_iv: Option<String>,
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
            image_data: row.image_data,
            image_iv: row.image_iv,
            thumbnails: Thumbnails {
                sm: ThumbnailData {
                    data: row.thumb_sm,
                    iv: row.thumb_sm_iv,
                },
                md: row.thumb_md.map(|data| ThumbnailData {
                    data,
                    iv: row.thumb_md_iv,
                }),
                lg: row.thumb_lg.map(|data| ThumbnailData {
                    data,
                    iv: row.thumb_lg_iv,
                }),
            },
            encryption_version: row.encryption_version,
            reference_count: row.reference_count,
            created_at: row.created_at,
        }
    }
}

/// Input for creating an attachment with multiple thumbnail sizes.
pub struct CreateAttachmentInput<'a> {
    pub user_id: i64,
    pub image_data: &'a [u8],
    /// IV for encryption, None if unencrypted
    pub image_iv: Option<&'a str>,
    pub thumb_sm: &'a [u8],
    /// IV for encryption, None if unencrypted
    pub thumb_sm_iv: Option<&'a str>,
    /// Medium thumbnail data and optional IV
    pub thumb_md: Option<(&'a [u8], Option<&'a str>)>,
    /// Large thumbnail data and optional IV
    pub thumb_lg: Option<(&'a [u8], Option<&'a str>)>,
    /// 0 = unencrypted, >0 = encrypted with that version
    pub encryption_version: i32,
}

impl AttachmentStore {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Create a new attachment with multiple thumbnail sizes. Returns the attachment UUID.
    /// Reference count starts at 0 (incremented when added to a post).
    pub async fn create(&self, input: CreateAttachmentInput<'_>) -> Result<String, sqlx::Error> {
        let uuid = uuid::Uuid::new_v4().to_string();

        sqlx::query(
            "INSERT INTO attachments (uuid, user_id, image_data, image_iv,
             thumb_sm, thumb_sm_iv,
             thumb_md, thumb_md_iv,
             thumb_lg, thumb_lg_iv,
             encryption_version, reference_count)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 0)",
        )
        .bind(&uuid)
        .bind(input.user_id)
        .bind(input.image_data)
        .bind(input.image_iv)
        .bind(input.thumb_sm)
        .bind(input.thumb_sm_iv)
        .bind(input.thumb_md.map(|(d, _)| d))
        .bind(input.thumb_md.and_then(|(_, iv)| iv))
        .bind(input.thumb_lg.map(|(d, _)| d))
        .bind(input.thumb_lg.and_then(|(_, iv)| iv))
        .bind(input.encryption_version)
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
            "SELECT id, uuid, user_id, image_data, image_iv,
             thumb_sm, thumb_sm_iv,
             thumb_md, thumb_md_iv,
             thumb_lg, thumb_lg_iv,
             encryption_version, reference_count, created_at
             FROM attachments WHERE uuid = ? AND user_id = ?",
        )
        .bind(uuid)
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(Attachment::from))
    }

    /// Get all thumbnails for an attachment.
    pub async fn get_thumbnails(
        &self,
        uuid: &str,
        user_id: i64,
    ) -> Result<Option<Thumbnails>, sqlx::Error> {
        let row: Option<(
            Vec<u8>,
            Option<String>,
            Option<Vec<u8>>,
            Option<String>,
            Option<Vec<u8>>,
            Option<String>,
        )> = sqlx::query_as(
            "SELECT thumb_sm, thumb_sm_iv,
             thumb_md, thumb_md_iv,
             thumb_lg, thumb_lg_iv
             FROM attachments WHERE uuid = ? AND user_id = ?",
        )
        .bind(uuid)
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(
            |(sm_data, sm_iv, md_data, md_iv, lg_data, lg_iv)| Thumbnails {
                sm: ThumbnailData {
                    data: sm_data,
                    iv: sm_iv,
                },
                md: md_data.map(|data| ThumbnailData { data, iv: md_iv }),
                lg: lg_data.map(|data| ThumbnailData { data, iv: lg_iv }),
            },
        ))
    }

    /// Get a single thumbnail by size for an attachment.
    /// Only fetches the requested size column from the database.
    /// Returns None if attachment doesn't exist or requested size is not available.
    pub async fn get_thumbnail_by_size(
        &self,
        uuid: &str,
        user_id: i64,
        size: &str,
    ) -> Result<Option<ThumbnailData>, sqlx::Error> {
        let row: Option<(Option<Vec<u8>>, Option<String>)> = match size {
            "sm" => {
                sqlx::query_as(
                    "SELECT thumb_sm, thumb_sm_iv
                     FROM attachments WHERE uuid = ? AND user_id = ?",
                )
                .bind(uuid)
                .bind(user_id)
                .fetch_optional(&self.pool)
                .await?
            }
            "md" => {
                sqlx::query_as(
                    "SELECT thumb_md, thumb_md_iv
                     FROM attachments WHERE uuid = ? AND user_id = ?",
                )
                .bind(uuid)
                .bind(user_id)
                .fetch_optional(&self.pool)
                .await?
            }
            "lg" => {
                sqlx::query_as(
                    "SELECT thumb_lg, thumb_lg_iv
                     FROM attachments WHERE uuid = ? AND user_id = ?",
                )
                .bind(uuid)
                .bind(user_id)
                .fetch_optional(&self.pool)
                .await?
            }
            _ => return Ok(None),
        };

        let Some((data, iv)) = row else {
            return Ok(None);
        };

        // Return thumbnail if data is present (IV may be None for unencrypted)
        Ok(data.map(|data| ThumbnailData { data, iv }))
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
    use super::*;
    use crate::db::Database;

    fn create_test_input(user_id: i64) -> CreateAttachmentInput<'static> {
        CreateAttachmentInput {
            user_id,
            image_data: b"image_data_bytes",
            image_iv: Some("image_iv_123"),
            thumb_sm: b"thumb_sm_data",
            thumb_sm_iv: Some("thumb_sm_iv"),
            thumb_md: Some((b"thumb_md_data", Some("thumb_md_iv"))),
            thumb_lg: Some((b"thumb_lg_data", Some("thumb_lg_iv"))),
            encryption_version: 1,
        }
    }

    #[tokio::test]
    async fn test_create_and_get_attachment() {
        let db = Database::open(":memory:").await.unwrap();
        let user_id = db.users().create("uuid-1", "alice").await.unwrap();

        let input = create_test_input(user_id);
        let attachment_uuid = db.attachments().create(input).await.unwrap();

        let attachment = db
            .attachments()
            .get_by_uuid(&attachment_uuid, user_id)
            .await
            .unwrap()
            .unwrap();

        assert_eq!(attachment.uuid, attachment_uuid);
        assert_eq!(attachment.user_id, user_id);
        assert_eq!(attachment.image_data, b"image_data_bytes");
        assert_eq!(attachment.image_iv, Some("image_iv_123".to_string()));
        assert_eq!(attachment.thumbnails.sm.data, b"thumb_sm_data");
        assert_eq!(attachment.thumbnails.sm.iv, Some("thumb_sm_iv".to_string()));
        assert_eq!(
            attachment.thumbnails.md.as_ref().unwrap().data,
            b"thumb_md_data"
        );
        assert_eq!(
            attachment.thumbnails.lg.as_ref().unwrap().data,
            b"thumb_lg_data"
        );
        assert_eq!(attachment.encryption_version, 1);
        assert_eq!(attachment.reference_count, 0);
    }

    #[tokio::test]
    async fn test_get_thumbnails() {
        let db = Database::open(":memory:").await.unwrap();
        let user_id = db.users().create("uuid-1", "alice").await.unwrap();

        let input = create_test_input(user_id);
        let attachment_uuid = db.attachments().create(input).await.unwrap();

        let thumbs = db
            .attachments()
            .get_thumbnails(&attachment_uuid, user_id)
            .await
            .unwrap()
            .unwrap();

        assert_eq!(thumbs.sm.data, b"thumb_sm_data");
        assert_eq!(thumbs.sm.iv, Some("thumb_sm_iv".to_string()));
        assert_eq!(thumbs.md.as_ref().unwrap().data, b"thumb_md_data");
        assert_eq!(thumbs.lg.as_ref().unwrap().data, b"thumb_lg_data");
    }

    #[tokio::test]
    async fn test_reference_counting() {
        let db = Database::open(":memory:").await.unwrap();
        let user_id = db.users().create("uuid-1", "alice").await.unwrap();

        let input = CreateAttachmentInput {
            user_id,
            image_data: b"img",
            image_iv: Some("iv1"),
            thumb_sm: b"thumb",
            thumb_sm_iv: Some("iv2"),
            thumb_md: None,
            thumb_lg: None,
            encryption_version: 1,
        };
        let uuid = db.attachments().create(input).await.unwrap();

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

        let input = CreateAttachmentInput {
            user_id: alice_id,
            image_data: b"img",
            image_iv: Some("iv1"),
            thumb_sm: b"thumb",
            thumb_sm_iv: Some("iv2"),
            thumb_md: None,
            thumb_lg: None,
            encryption_version: 1,
        };
        let uuid = db.attachments().create(input).await.unwrap();

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
        let input1 = CreateAttachmentInput {
            user_id,
            image_data: b"img1",
            image_iv: Some("iv1"),
            thumb_sm: b"thumb1",
            thumb_sm_iv: Some("iv1t"),
            thumb_md: None,
            thumb_lg: None,
            encryption_version: 1,
        };
        let input2 = CreateAttachmentInput {
            user_id,
            image_data: b"img2",
            image_iv: Some("iv2"),
            thumb_sm: b"thumb2",
            thumb_sm_iv: Some("iv2t"),
            thumb_md: None,
            thumb_lg: None,
            encryption_version: 1,
        };
        let att1_uuid = db.attachments().create(input1).await.unwrap();
        let att2_uuid = db.attachments().create(input2).await.unwrap();

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
        let input = CreateAttachmentInput {
            user_id,
            image_data: b"img",
            image_iv: Some("iv"),
            thumb_sm: b"thumb",
            thumb_sm_iv: Some("ivt"),
            thumb_md: None,
            thumb_lg: None,
            encryption_version: 1,
        };
        let att_uuid = db.attachments().create(input).await.unwrap();

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
