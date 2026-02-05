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
use crate::auth::{AnyRole, Auth};
use crate::cli::IpExtractor;
use crate::db::{Database, PostNode, UpdatePostParams};
use crate::impl_has_auth_state;
use crate::jwt::JwtConfig;

/// State for posts endpoints.
#[derive(Clone)]
pub struct PostsState {
    pub db: Database,
    pub jwt: Arc<JwtConfig>,
    pub secure_cookies: bool,
    pub ip_extractor: Option<IpExtractor>,
}

impl_has_auth_state!(PostsState);

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
    1
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
    auth: Auth<AnyRole>,
    Query(query): Query<ListPostsQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let posts = state
        .db
        .posts()
        .list_tree(auth.user_id, query.depth)
        .await
        .db_err("Failed to list posts")?;

    let response: Vec<PostNodeResponse> = posts.into_iter().map(Into::into).collect();

    Ok(Json(response))
}

async fn list_children(
    State(state): State<PostsState>,
    auth: Auth<AnyRole>,
    Path(uuid): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let children = state
        .db
        .posts()
        .list_children(auth.user_id, &uuid)
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
    auth: Auth<AnyRole>,
    Json(payload): Json<CreatePostRequest>,
) -> Result<impl IntoResponse, ApiError> {
    // Validate encryption matches user settings
    validate_encryption(
        &state.db,
        auth.user_id,
        &payload.encryption_version,
        payload.content_encrypted,
    )
    .await?;

    let uuid = state
        .db
        .posts()
        .create(
            auth.user_id,
            payload.title.as_deref(),
            payload.title_encrypted,
            payload.title_iv.as_deref(),
            &payload.content,
            payload.content_encrypted,
            payload.iv.as_deref(),
            payload.encryption_version,
            payload.parent_id.as_deref(),
        )
        .await
        .map_err(|e| match e {
            sqlx::Error::RowNotFound => ApiError::not_found("Parent post not found"),
            _ => ApiError::internal("Failed to create post"),
        })?;

    let post = state
        .db
        .posts()
        .get_by_uuid(&uuid, auth.user_id)
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
            created_at: post.created_at,
            updated_at: post.updated_at,
        }),
    ))
}

async fn get_post(
    State(state): State<PostsState>,
    auth: Auth<AnyRole>,
    Path(uuid): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let post = state
        .db
        .posts()
        .get_by_uuid(&uuid, auth.user_id)
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
        created_at: post.created_at,
        updated_at: post.updated_at,
    }))
}

async fn update_post(
    State(state): State<PostsState>,
    auth: Auth<AnyRole>,
    Path(uuid): Path<String>,
    Json(payload): Json<UpdatePostRequest>,
) -> Result<impl IntoResponse, ApiError> {
    // Validate encryption matches user settings
    validate_encryption(
        &state.db,
        auth.user_id,
        &payload.encryption_version,
        payload.content_encrypted,
    )
    .await?;

    let found = state
        .db
        .update_post_with_attachments(UpdatePostParams {
            uuid: &uuid,
            user_id: auth.user_id,
            title: payload.title.as_deref(),
            title_encrypted: payload.title_encrypted,
            title_iv: payload.title_iv.as_deref(),
            content: &payload.content,
            content_encrypted: payload.content_encrypted,
            iv: payload.iv.as_deref(),
            encryption_version: payload.encryption_version,
            attachment_uuids: payload.attachment_uuids.as_deref(),
        })
        .await
        .db_err("Failed to update post")?;

    if !found {
        return Err(ApiError::not_found("Post not found"));
    }

    Ok(StatusCode::NO_CONTENT)
}

async fn delete_post(
    State(state): State<PostsState>,
    auth: Auth<AnyRole>,
    Path(uuid): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let result = state
        .db
        .delete_post_with_attachments(&uuid, auth.user_id)
        .await
        .db_err("Failed to delete post")?;

    if !result.deleted {
        return Err(ApiError::not_found("Post not found"));
    }

    Ok(Json(DeleteResponse {
        deleted: true,
        children_deleted: result.children_deleted,
    }))
}

async fn reorder_posts(
    State(state): State<PostsState>,
    auth: Auth<AnyRole>,
    Json(payload): Json<ReorderRequest>,
) -> Result<impl IntoResponse, ApiError> {
    state
        .db
        .posts()
        .reorder(auth.user_id, payload.parent_id.as_deref(), &payload.uuids)
        .await
        .db_err("Failed to reorder posts")?;

    Ok(StatusCode::NO_CONTENT)
}

async fn move_post(
    State(state): State<PostsState>,
    auth: Auth<AnyRole>,
    Path(uuid): Path<String>,
    Json(payload): Json<MovePostRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let moved = state
        .db
        .posts()
        .move_post(
            &uuid,
            auth.user_id,
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
