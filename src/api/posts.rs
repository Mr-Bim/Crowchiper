//! Posts API for post entries with hierarchical structure support.
//!
//! All endpoints require JWT authentication.

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, post, put},
};
use serde::{Deserialize, Serialize};
use sqlx;
use std::sync::Arc;

use super::error::{ApiError, ResultExt};
use crate::auth::{ApiAuth, HasAuthState};
use crate::db::{Database, PostNode};
use crate::jwt::JwtConfig;

/// State for posts endpoints.
#[derive(Clone)]
pub struct PostsState {
    pub db: Database,
    pub jwt: Arc<JwtConfig>,
}

impl HasAuthState for PostsState {
    fn jwt(&self) -> &JwtConfig {
        &self.jwt
    }
    fn db(&self) -> &Database {
        &self.db
    }
}

pub fn router(state: PostsState) -> Router {
    Router::new()
        .route("/", get(list_posts))
        .route("/", post(create_post))
        .route("/reorder", post(reorder_posts))
        .route("/{uuid}", get(get_post))
        // Accept both PUT (normal update) and POST (sendBeacon on page unload)
        .route("/{uuid}", put(update_post).post(update_post))
        .route("/{uuid}", delete(delete_post))
        .route("/{uuid}/children", get(list_children))
        .route("/{uuid}/move", post(move_post))
        .with_state(state)
}

// --- Request/Response types ---

#[derive(Deserialize)]
struct ListPostsQuery {
    #[serde(default = "default_depth")]
    depth: i32,
}

fn default_depth() -> i32 {
    3
}

#[derive(Deserialize)]
struct CreatePostRequest {
    title: Option<String>,
    #[serde(default)]
    title_encrypted: bool,
    title_iv: Option<String>,
    #[serde(default)]
    content: String,
    #[serde(default)]
    content_encrypted: bool,
    iv: Option<String>,
    encryption_version: Option<i32>,
    parent_id: Option<String>,
    #[serde(default)]
    is_folder: bool,
}

#[derive(Serialize)]
struct PostResponse {
    uuid: String,
    title: Option<String>,
    title_encrypted: bool,
    title_iv: Option<String>,
    content: String,
    content_encrypted: bool,
    iv: Option<String>,
    encryption_version: Option<i32>,
    position: Option<i32>,
    parent_id: Option<String>,
    is_folder: bool,
    created_at: String,
    updated_at: String,
}

#[derive(Serialize)]
struct PostNodeResponse {
    uuid: String,
    title: Option<String>,
    title_encrypted: bool,
    title_iv: Option<String>,
    content_encrypted: bool,
    encryption_version: Option<i32>,
    position: Option<i32>,
    parent_id: Option<String>,
    is_folder: bool,
    has_children: bool,
    children: Option<Vec<PostNodeResponse>>,
    created_at: String,
    updated_at: String,
}

impl From<PostNode> for PostNodeResponse {
    fn from(node: PostNode) -> Self {
        Self {
            uuid: node.uuid,
            title: node.title,
            title_encrypted: node.title_encrypted,
            title_iv: node.title_iv,
            content_encrypted: node.content_encrypted,
            encryption_version: node.encryption_version,
            position: node.position,
            parent_id: node.parent_id,
            is_folder: node.is_folder,
            has_children: node.has_children,
            children: node
                .children
                .map(|c| c.into_iter().map(Into::into).collect()),
            created_at: node.created_at,
            updated_at: node.updated_at,
        }
    }
}

#[derive(Deserialize)]
struct ReorderRequest {
    parent_id: Option<String>,
    uuids: Vec<String>,
}

#[derive(Deserialize)]
struct UpdatePostRequest {
    title: Option<String>,
    #[serde(default)]
    title_encrypted: bool,
    title_iv: Option<String>,
    content: String,
    #[serde(default)]
    content_encrypted: bool,
    iv: Option<String>,
    encryption_version: Option<i32>,
    /// Optional attachment UUIDs to update refs (used with sendBeacon on page unload)
    attachment_uuids: Option<Vec<String>>,
}

#[derive(Deserialize)]
struct MovePostRequest {
    parent_id: Option<String>,
    position: i32,
}

#[derive(Serialize)]
struct DeleteResponse {
    deleted: bool,
    children_deleted: i64,
}

// --- Helpers ---

/// Validate that encryption settings match user's configuration.
/// - If user has encryption enabled, content must be encrypted (encryption_version > 0)
/// - If user does not have encryption enabled, content must be unencrypted (encryption_version = 0 or None)
async fn validate_encryption(
    db: &Database,
    user_id: i64,
    encryption_version: &Option<i32>,
    content_encrypted: bool,
) -> Result<(), ApiError> {
    let encryption_settings = db
        .encryption_settings()
        .get(user_id)
        .await
        .db_err("Failed to get encryption settings")?;

    let user_has_encryption = encryption_settings
        .map(|s| s.encryption_enabled)
        .unwrap_or(false);

    let is_encrypted = encryption_version.map(|v| v > 0).unwrap_or(false) || content_encrypted;

    if user_has_encryption && !is_encrypted {
        return Err(ApiError::bad_request(
            "Encryption is enabled but unencrypted content was submitted",
        ));
    }

    if !user_has_encryption && is_encrypted {
        return Err(ApiError::bad_request(
            "Encryption is not enabled but encrypted content was submitted",
        ));
    }

    Ok(())
}

// --- Handlers ---

async fn list_posts(
    State(state): State<PostsState>,
    ApiAuth(user): ApiAuth,
    Query(query): Query<ListPostsQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let posts = state
        .db
        .posts()
        .list_tree(user.user_id, query.depth)
        .await
        .db_err("Failed to list posts")?;

    let response: Vec<PostNodeResponse> = posts.into_iter().map(Into::into).collect();

    Ok(Json(response))
}

async fn list_children(
    State(state): State<PostsState>,
    ApiAuth(user): ApiAuth,
    Path(uuid): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let children = state
        .db
        .posts()
        .list_children(user.user_id, &uuid)
        .await
        .map_err(|e| match e {
            sqlx::Error::RowNotFound => ApiError::not_found("Parent post not found"),
            _ => ApiError::internal("Failed to list children"),
        })?;

    let response: Vec<PostNodeResponse> = children.into_iter().map(Into::into).collect();

    Ok(Json(response))
}

async fn create_post(
    State(state): State<PostsState>,
    ApiAuth(user): ApiAuth,
    Json(payload): Json<CreatePostRequest>,
) -> Result<impl IntoResponse, ApiError> {
    // Validate encryption matches user settings
    validate_encryption(
        &state.db,
        user.user_id,
        &payload.encryption_version,
        payload.content_encrypted,
    )
    .await?;

    let uuid = state
        .db
        .posts()
        .create(
            user.user_id,
            payload.title.as_deref(),
            payload.title_encrypted,
            payload.title_iv.as_deref(),
            &payload.content,
            payload.content_encrypted,
            payload.iv.as_deref(),
            payload.encryption_version,
            payload.parent_id.as_deref(),
            payload.is_folder,
        )
        .await
        .map_err(|e| match e {
            sqlx::Error::RowNotFound => ApiError::not_found("Parent post not found"),
            _ => ApiError::internal("Failed to create post"),
        })?;

    let post = state
        .db
        .posts()
        .get_by_uuid(&uuid, user.user_id)
        .await
        .db_err("Failed to get created post")?
        .ok_or_else(|| ApiError::internal("Created post not found"))?;

    Ok((
        StatusCode::CREATED,
        Json(PostResponse {
            uuid: post.uuid,
            title: post.title,
            title_encrypted: post.title_encrypted,
            title_iv: post.title_iv,
            content: post.content,
            content_encrypted: post.content_encrypted,
            iv: post.iv,
            encryption_version: post.encryption_version,
            position: post.position,
            parent_id: post.parent_id,
            is_folder: post.is_folder,
            created_at: post.created_at,
            updated_at: post.updated_at,
        }),
    ))
}

async fn get_post(
    State(state): State<PostsState>,
    ApiAuth(user): ApiAuth,
    Path(uuid): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let post = state
        .db
        .posts()
        .get_by_uuid(&uuid, user.user_id)
        .await
        .db_err("Failed to get post")?
        .ok_or_else(|| ApiError::not_found("Post not found"))?;

    Ok(Json(PostResponse {
        uuid: post.uuid,
        title: post.title,
        title_encrypted: post.title_encrypted,
        title_iv: post.title_iv,
        content: post.content,
        content_encrypted: post.content_encrypted,
        iv: post.iv,
        encryption_version: post.encryption_version,
        position: post.position,
        parent_id: post.parent_id,
        is_folder: post.is_folder,
        created_at: post.created_at,
        updated_at: post.updated_at,
    }))
}

async fn update_post(
    State(state): State<PostsState>,
    ApiAuth(user): ApiAuth,
    Path(uuid): Path<String>,
    Json(payload): Json<UpdatePostRequest>,
) -> Result<impl IntoResponse, ApiError> {
    // Validate encryption matches user settings
    validate_encryption(
        &state.db,
        user.user_id,
        &payload.encryption_version,
        payload.content_encrypted,
    )
    .await?;

    // Use a transaction to ensure atomicity when updating post and attachment refs
    let mut tx = state
        .db
        .begin()
        .await
        .db_err("Failed to begin transaction")?;

    // Get the post first to verify ownership and get post_id for attachment refs
    let post: Option<(i64,)> =
        sqlx::query_as("SELECT id FROM posts WHERE uuid = ? AND user_id = ?")
            .bind(&uuid)
            .bind(user.user_id)
            .fetch_optional(&mut *tx)
            .await
            .db_err("Failed to get post")?;

    let post_id = post.ok_or_else(|| ApiError::not_found("Post not found"))?.0;

    // Update the post
    sqlx::query(
        "UPDATE posts SET title = ?, title_encrypted = ?, title_iv = ?, content = ?, content_encrypted = ?, iv = ?, encryption_version = ?, updated_at = datetime('now')
         WHERE uuid = ? AND user_id = ?",
    )
    .bind(payload.title.as_deref())
    .bind(payload.title_encrypted)
    .bind(payload.title_iv.as_deref())
    .bind(&payload.content)
    .bind(payload.content_encrypted)
    .bind(payload.iv.as_deref())
    .bind(payload.encryption_version)
    .bind(&uuid)
    .bind(user.user_id)
    .execute(&mut *tx)
    .await
    .db_err("Failed to update post")?;

    // Update attachment refs if provided (used with sendBeacon on page unload)
    if let Some(attachment_uuids) = payload.attachment_uuids {
        update_post_attachments_tx(&mut tx, post_id, user.user_id, &attachment_uuids)
            .await
            .db_err("Failed to update attachment references")?;
    }

    tx.commit().await.db_err("Failed to commit transaction")?;

    Ok(StatusCode::NO_CONTENT)
}

/// Update attachment references within a transaction.
async fn update_post_attachments_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    post_id: i64,
    user_id: i64,
    new_uuids: &[String],
) -> Result<(), sqlx::Error> {
    use std::collections::HashSet;

    // Get current attachments for this post
    let current_rows: Vec<(String,)> =
        sqlx::query_as("SELECT attachment_uuid FROM post_attachments WHERE post_id = ?")
            .bind(post_id)
            .fetch_all(&mut **tx)
            .await?;
    let current: HashSet<String> = current_rows.into_iter().map(|r| r.0).collect();

    let new_set: HashSet<String> = new_uuids.iter().cloned().collect();

    // Find removed attachments (in current but not in new)
    for uuid in current.difference(&new_set) {
        // Decrement reference count
        sqlx::query(
            "UPDATE attachments SET reference_count = reference_count - 1
             WHERE uuid = ? AND user_id = ? AND reference_count > 0",
        )
        .bind(uuid)
        .bind(user_id)
        .execute(&mut **tx)
        .await?;

        // Delete if reference count is now 0
        sqlx::query(
            "DELETE FROM attachments WHERE uuid = ? AND user_id = ? AND reference_count = 0",
        )
        .bind(uuid)
        .bind(user_id)
        .execute(&mut **tx)
        .await?;

        // Remove from post_attachments
        sqlx::query("DELETE FROM post_attachments WHERE post_id = ? AND attachment_uuid = ?")
            .bind(post_id)
            .bind(uuid)
            .execute(&mut **tx)
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
        .execute(&mut **tx)
        .await?;

        // Add to post_attachments
        sqlx::query(
            "INSERT OR IGNORE INTO post_attachments (post_id, attachment_uuid) VALUES (?, ?)",
        )
        .bind(post_id)
        .bind(uuid)
        .execute(&mut **tx)
        .await?;
    }

    Ok(())
}

async fn delete_post(
    State(state): State<PostsState>,
    ApiAuth(user): ApiAuth,
    Path(uuid): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    // Use a transaction to ensure atomicity when deleting post and cleaning up attachments
    let mut tx = state
        .db
        .begin()
        .await
        .db_err("Failed to begin transaction")?;

    // Get the post first to verify ownership and get post_id for attachment cleanup
    let post: Option<(i64,)> =
        sqlx::query_as("SELECT id FROM posts WHERE uuid = ? AND user_id = ?")
            .bind(&uuid)
            .bind(user.user_id)
            .fetch_optional(&mut *tx)
            .await
            .db_err("Failed to get post")?;

    // Verify post exists and belongs to user
    post.ok_or_else(|| ApiError::not_found("Post not found"))?;

    // Count descendants first (for response)
    let children_count = state
        .db
        .posts()
        .count_descendants(&uuid, user.user_id)
        .await
        .db_err("Failed to count descendants")?;

    // Get all descendant post IDs for attachment cleanup
    let descendant_ids: Vec<(i64,)> = sqlx::query_as(
        "WITH RECURSIVE descendants AS (
            SELECT id FROM posts WHERE uuid = ? AND user_id = ?
            UNION ALL
            SELECT p.id FROM posts p
            INNER JOIN descendants d ON p.parent_id = (SELECT uuid FROM posts WHERE id = d.id)
            WHERE p.user_id = ?
        )
        SELECT id FROM descendants",
    )
    .bind(&uuid)
    .bind(user.user_id)
    .bind(user.user_id)
    .fetch_all(&mut *tx)
    .await
    .db_err("Failed to get descendant IDs")?;

    // Remove attachment references for all posts being deleted
    for (id,) in descendant_ids {
        remove_post_attachments_tx(&mut tx, id, user.user_id)
            .await
            .db_err("Failed to remove attachment references")?;
    }

    // Delete the post (children cascade via database constraint)
    sqlx::query("DELETE FROM posts WHERE uuid = ? AND user_id = ?")
        .bind(&uuid)
        .bind(user.user_id)
        .execute(&mut *tx)
        .await
        .db_err("Failed to delete post")?;

    tx.commit().await.db_err("Failed to commit transaction")?;

    Ok(Json(DeleteResponse {
        deleted: true,
        children_deleted: children_count,
    }))
}

/// Remove all attachment references for a post within a transaction.
async fn remove_post_attachments_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    post_id: i64,
    user_id: i64,
) -> Result<(), sqlx::Error> {
    // Get current attachments for this post
    let current_rows: Vec<(String,)> =
        sqlx::query_as("SELECT attachment_uuid FROM post_attachments WHERE post_id = ?")
            .bind(post_id)
            .fetch_all(&mut **tx)
            .await?;

    // Decrement ref count for each and delete if 0
    for (uuid,) in current_rows {
        sqlx::query(
            "UPDATE attachments SET reference_count = reference_count - 1
             WHERE uuid = ? AND user_id = ? AND reference_count > 0",
        )
        .bind(&uuid)
        .bind(user_id)
        .execute(&mut **tx)
        .await?;

        sqlx::query(
            "DELETE FROM attachments WHERE uuid = ? AND user_id = ? AND reference_count = 0",
        )
        .bind(&uuid)
        .bind(user_id)
        .execute(&mut **tx)
        .await?;
    }

    // Remove all post_attachments entries for this post
    sqlx::query("DELETE FROM post_attachments WHERE post_id = ?")
        .bind(post_id)
        .execute(&mut **tx)
        .await?;

    Ok(())
}

async fn reorder_posts(
    State(state): State<PostsState>,
    ApiAuth(user): ApiAuth,
    Json(payload): Json<ReorderRequest>,
) -> Result<impl IntoResponse, ApiError> {
    state
        .db
        .posts()
        .reorder(user.user_id, payload.parent_id.as_deref(), &payload.uuids)
        .await
        .db_err("Failed to reorder posts")?;

    Ok(StatusCode::NO_CONTENT)
}

async fn move_post(
    State(state): State<PostsState>,
    ApiAuth(user): ApiAuth,
    Path(uuid): Path<String>,
    Json(payload): Json<MovePostRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let moved = state
        .db
        .posts()
        .move_post(
            &uuid,
            user.user_id,
            payload.parent_id.as_deref(),
            payload.position,
        )
        .await
        .map_err(|e| match e {
            sqlx::Error::RowNotFound => ApiError::not_found("Post or parent not found"),
            sqlx::Error::Protocol(msg) if msg.contains("descendant") => {
                ApiError::bad_request("Cannot move a post to its own descendant")
            }
            _ => ApiError::internal("Failed to move post"),
        })?;

    if !moved {
        return Err(ApiError::not_found("Post not found"));
    }

    Ok(StatusCode::NO_CONTENT)
}
