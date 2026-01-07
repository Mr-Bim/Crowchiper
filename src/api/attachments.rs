//! Attachments API for encrypted image uploads.
//!
//! All endpoints require JWT authentication.

use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use super::error::{ApiError, ResultExt};
use crate::auth::{ApiAuth, HasAuthState};
use crate::db::Database;
use crate::jwt::JwtConfig;

/// State for attachments endpoints.
#[derive(Clone)]
pub struct AttachmentsState {
    pub db: Database,
    pub jwt: Arc<JwtConfig>,
}

impl HasAuthState for AttachmentsState {
    fn jwt(&self) -> &JwtConfig {
        &self.jwt
    }
    fn db(&self) -> &Database {
        &self.db
    }
}

pub fn router(state: AttachmentsState) -> Router {
    Router::new()
        .route("/", post(upload_attachment))
        .route("/{uuid}", get(get_attachment))
        .route("/{uuid}/thumbnail", get(get_thumbnail))
        // 15MB limit: 10MB image + 100KB thumbnail + base64 overhead (~33%)
        .layer(DefaultBodyLimit::max(15 * 1024 * 1024))
        .with_state(state)
}

// --- Request/Response types ---

#[derive(Deserialize)]
struct UploadRequest {
    /// Base64url-encoded encrypted image
    encrypted_image: String,
    /// IV for encrypted image
    encrypted_image_iv: String,
    /// Base64url-encoded encrypted thumbnail
    encrypted_thumbnail: String,
    /// IV for encrypted thumbnail
    encrypted_thumbnail_iv: String,
    /// Encryption version
    encryption_version: i32,
}

#[derive(Serialize)]
struct UploadResponse {
    uuid: String,
}

#[derive(Serialize)]
struct AttachmentResponse {
    /// Base64url-encoded encrypted image
    encrypted_image: String,
    /// IV for encrypted image
    iv: String,
}

#[derive(Serialize)]
struct ThumbnailResponse {
    /// Base64url-encoded encrypted thumbnail
    encrypted_thumbnail: String,
    /// IV for encrypted thumbnail
    iv: String,
}

// --- Handlers ---

async fn upload_attachment(
    State(state): State<AttachmentsState>,
    ApiAuth(user): ApiAuth,
    Json(payload): Json<UploadRequest>,
) -> Result<impl IntoResponse, ApiError> {
    // Decode base64url-encoded image data
    let encrypted_image = URL_SAFE_NO_PAD
        .decode(&payload.encrypted_image)
        .map_err(|_| ApiError::bad_request("Invalid base64url encoding for encrypted_image"))?;

    let encrypted_thumbnail = URL_SAFE_NO_PAD
        .decode(&payload.encrypted_thumbnail)
        .map_err(|_| ApiError::bad_request("Invalid base64url encoding for encrypted_thumbnail"))?;

    // Limit image size to 10MB
    if encrypted_image.len() > 10 * 1024 * 1024 {
        return Err(ApiError::bad_request("Image too large (max 10MB)"));
    }

    // Limit thumbnail size to 100KB
    if encrypted_thumbnail.len() > 100 * 1024 {
        return Err(ApiError::bad_request("Thumbnail too large (max 100KB)"));
    }

    let uuid = state
        .db
        .attachments()
        .create(
            user.user_id,
            &encrypted_image,
            &payload.encrypted_image_iv,
            &encrypted_thumbnail,
            &payload.encrypted_thumbnail_iv,
            payload.encryption_version,
        )
        .await
        .db_err("Failed to create attachment")?;

    Ok((StatusCode::CREATED, Json(UploadResponse { uuid })))
}

async fn get_attachment(
    State(state): State<AttachmentsState>,
    ApiAuth(user): ApiAuth,
    Path(uuid): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let attachment = state
        .db
        .attachments()
        .get_by_uuid(&uuid, user.user_id)
        .await
        .db_err("Failed to get attachment")?
        .ok_or_else(|| ApiError::not_found("Attachment not found"))?;

    let encoded_image = URL_SAFE_NO_PAD.encode(&attachment.encrypted_image);

    Ok(Json(AttachmentResponse {
        encrypted_image: encoded_image,
        iv: attachment.encrypted_image_iv,
    }))
}

async fn get_thumbnail(
    State(state): State<AttachmentsState>,
    ApiAuth(user): ApiAuth,
    Path(uuid): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let (thumbnail_data, iv) = state
        .db
        .attachments()
        .get_thumbnail(&uuid, user.user_id)
        .await
        .db_err("Failed to get thumbnail")?
        .ok_or_else(|| ApiError::not_found("Attachment not found"))?;

    let encoded_thumbnail = URL_SAFE_NO_PAD.encode(&thumbnail_data);

    Ok(Json(ThumbnailResponse {
        encrypted_thumbnail: encoded_thumbnail,
        iv,
    }))
}
