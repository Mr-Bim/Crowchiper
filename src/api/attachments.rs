//! Attachments API for encrypted image uploads.
//!
//! All endpoints require JWT authentication.
//! Uses binary streaming instead of base64 for efficiency.

use axum::{
    Router,
    body::Body,
    extract::{DefaultBodyLimit, Multipart, Path, State},
    http::{HeaderMap, StatusCode, header},
    response::IntoResponse,
    routing::{get, post},
};
use serde::Serialize;
use std::sync::Arc;

use super::error::{ApiError, ResultExt};
use crate::auth::{ApiAuth, HasAuthState};
use crate::db::{Database, attachments::CreateAttachmentInput};
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
        .route("/{uuid}/thumbnails", get(get_thumbnails))
        .route("/{uuid}/thumbnail/{size}", get(get_thumbnail))
        // 15MB limit: 10MB image + thumbnails + overhead
        .layer(DefaultBodyLimit::max(15 * 1024 * 1024))
        .with_state(state)
}

// --- Response types ---

#[derive(Serialize)]
struct UploadResponse {
    uuid: String,
}

// --- Handlers ---

/// Upload an encrypted attachment using multipart form data.
///
/// Expected fields:
/// - `image`: Binary encrypted image data
/// - `image_iv`: IV for image (base64url string)
/// - `thumb_sm`: Binary encrypted small thumbnail (200px)
/// - `thumb_sm_iv`: IV for small thumbnail
/// - `thumb_md`: Binary encrypted medium thumbnail (400px) - optional
/// - `thumb_md_iv`: IV for medium thumbnail - optional
/// - `thumb_lg`: Binary encrypted large thumbnail (800px) - optional
/// - `thumb_lg_iv`: IV for large thumbnail - optional
/// - `encryption_version`: Version number (string, parsed as i32)
async fn upload_attachment(
    State(state): State<AttachmentsState>,
    ApiAuth(user): ApiAuth,
    mut multipart: Multipart,
) -> Result<impl IntoResponse, ApiError> {
    let mut encrypted_image: Option<Vec<u8>> = None;
    let mut encrypted_image_iv: Option<String> = None;
    let mut thumb_sm: Option<Vec<u8>> = None;
    let mut thumb_sm_iv: Option<String> = None;
    let mut thumb_md: Option<Vec<u8>> = None;
    let mut thumb_md_iv: Option<String> = None;
    let mut thumb_lg: Option<Vec<u8>> = None;
    let mut thumb_lg_iv: Option<String> = None;
    let mut encryption_version: Option<i32> = None;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|_| ApiError::bad_request("Invalid multipart data"))?
    {
        let name = field.name().unwrap_or("").to_string();
        match name.as_str() {
            "image" => {
                let data = field
                    .bytes()
                    .await
                    .map_err(|_| ApiError::bad_request("Failed to read image data"))?;
                encrypted_image = Some(data.to_vec());
            }
            "image_iv" => {
                let text = field
                    .text()
                    .await
                    .map_err(|_| ApiError::bad_request("Failed to read image_iv"))?;
                encrypted_image_iv = Some(text);
            }
            "thumb_sm" => {
                let data = field
                    .bytes()
                    .await
                    .map_err(|_| ApiError::bad_request("Failed to read thumb_sm data"))?;
                thumb_sm = Some(data.to_vec());
            }
            "thumb_sm_iv" => {
                let text = field
                    .text()
                    .await
                    .map_err(|_| ApiError::bad_request("Failed to read thumb_sm_iv"))?;
                thumb_sm_iv = Some(text);
            }
            "thumb_md" => {
                let data = field
                    .bytes()
                    .await
                    .map_err(|_| ApiError::bad_request("Failed to read thumb_md data"))?;
                thumb_md = Some(data.to_vec());
            }
            "thumb_md_iv" => {
                let text = field
                    .text()
                    .await
                    .map_err(|_| ApiError::bad_request("Failed to read thumb_md_iv"))?;
                thumb_md_iv = Some(text);
            }
            "thumb_lg" => {
                let data = field
                    .bytes()
                    .await
                    .map_err(|_| ApiError::bad_request("Failed to read thumb_lg data"))?;
                thumb_lg = Some(data.to_vec());
            }
            "thumb_lg_iv" => {
                let text = field
                    .text()
                    .await
                    .map_err(|_| ApiError::bad_request("Failed to read thumb_lg_iv"))?;
                thumb_lg_iv = Some(text);
            }
            "encryption_version" => {
                let text = field
                    .text()
                    .await
                    .map_err(|_| ApiError::bad_request("Failed to read encryption_version"))?;
                encryption_version = Some(
                    text.parse()
                        .map_err(|_| ApiError::bad_request("Invalid encryption_version"))?,
                );
            }
            _ => {
                // Ignore unknown fields
            }
        }
    }

    let encrypted_image =
        encrypted_image.ok_or_else(|| ApiError::bad_request("Missing image field"))?;
    let encrypted_image_iv =
        encrypted_image_iv.ok_or_else(|| ApiError::bad_request("Missing image_iv field"))?;
    let thumb_sm = thumb_sm.ok_or_else(|| ApiError::bad_request("Missing thumb_sm field"))?;
    let thumb_sm_iv =
        thumb_sm_iv.ok_or_else(|| ApiError::bad_request("Missing thumb_sm_iv field"))?;
    let encryption_version = encryption_version
        .ok_or_else(|| ApiError::bad_request("Missing encryption_version field"))?;

    // Limit image size to 10MB
    if encrypted_image.len() > 10 * 1024 * 1024 {
        return Err(ApiError::bad_request("Image too large (max 10MB)"));
    }

    // Limit thumbnail sizes
    if thumb_sm.len() > 100 * 1024 {
        return Err(ApiError::bad_request(
            "Small thumbnail too large (max 100KB)",
        ));
    }
    if let Some(ref data) = thumb_md {
        if data.len() > 200 * 1024 {
            return Err(ApiError::bad_request(
                "Medium thumbnail too large (max 200KB)",
            ));
        }
    }
    if let Some(ref data) = thumb_lg {
        if data.len() > 500 * 1024 {
            return Err(ApiError::bad_request(
                "Large thumbnail too large (max 500KB)",
            ));
        }
    }

    let input = CreateAttachmentInput {
        user_id: user.user_id,
        encrypted_image: &encrypted_image,
        encrypted_image_iv: &encrypted_image_iv,
        thumb_sm: &thumb_sm,
        thumb_sm_iv: &thumb_sm_iv,
        thumb_md: thumb_md
            .as_ref()
            .zip(thumb_md_iv.as_ref())
            .map(|(d, iv)| (d.as_slice(), iv.as_str())),
        thumb_lg: thumb_lg
            .as_ref()
            .zip(thumb_lg_iv.as_ref())
            .map(|(d, iv)| (d.as_slice(), iv.as_str())),
        encryption_version,
    };

    let uuid = state
        .db
        .attachments()
        .create(input)
        .await
        .db_err("Failed to create attachment")?;

    Ok((StatusCode::CREATED, axum::Json(UploadResponse { uuid })))
}

/// Get an encrypted attachment as binary stream.
/// IV is returned in the `X-Encryption-IV` header.
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

    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        "application/octet-stream".parse().unwrap(),
    );
    headers.insert(
        "X-Encryption-IV",
        attachment.encrypted_image_iv.parse().unwrap(),
    );

    Ok((headers, Body::from(attachment.encrypted_image)))
}

/// Get a single encrypted thumbnail by size as binary stream.
/// IV is returned in the `X-Encryption-IV` header.
/// Size must be "sm", "md", or "lg".
async fn get_thumbnail(
    State(state): State<AttachmentsState>,
    ApiAuth(user): ApiAuth,
    Path((uuid, size)): Path<(String, String)>,
) -> Result<impl IntoResponse, ApiError> {
    // Validate size parameter
    if !matches!(size.as_str(), "sm" | "md" | "lg") {
        return Err(ApiError::bad_request("Invalid size. Must be sm, md, or lg"));
    }

    let thumbnail = state
        .db
        .attachments()
        .get_thumbnail_by_size(&uuid, user.user_id, &size)
        .await
        .db_err("Failed to get thumbnail")?
        .ok_or_else(|| ApiError::not_found("Thumbnail not found"))?;

    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        "application/octet-stream".parse().unwrap(),
    );
    headers.insert("X-Encryption-IV", thumbnail.iv.parse().unwrap());

    Ok((headers, Body::from(thumbnail.data)))
}

/// Get all encrypted thumbnails as a multipart response.
/// Each part has `X-Thumbnail-Size` header (sm, md, lg) and `X-Encryption-IV` header.
async fn get_thumbnails(
    State(state): State<AttachmentsState>,
    ApiAuth(user): ApiAuth,
    Path(uuid): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let thumbnails = state
        .db
        .attachments()
        .get_thumbnails(&uuid, user.user_id)
        .await
        .db_err("Failed to get thumbnails")?
        .ok_or_else(|| ApiError::not_found("Attachment not found"))?;

    // Build multipart response
    let boundary = "----ThumbnailBoundary";
    let mut body = Vec::new();

    // Small thumbnail (always present)
    body.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
    body.extend_from_slice(b"Content-Type: application/octet-stream\r\n");
    body.extend_from_slice(format!("X-Thumbnail-Size: sm\r\n").as_bytes());
    body.extend_from_slice(format!("X-Encryption-IV: {}\r\n\r\n", thumbnails.sm.iv).as_bytes());
    body.extend_from_slice(&thumbnails.sm.data);
    body.extend_from_slice(b"\r\n");

    // Medium thumbnail (optional)
    if let Some(ref md) = thumbnails.md {
        body.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
        body.extend_from_slice(b"Content-Type: application/octet-stream\r\n");
        body.extend_from_slice(format!("X-Thumbnail-Size: md\r\n").as_bytes());
        body.extend_from_slice(format!("X-Encryption-IV: {}\r\n\r\n", md.iv).as_bytes());
        body.extend_from_slice(&md.data);
        body.extend_from_slice(b"\r\n");
    }

    // Large thumbnail (optional)
    if let Some(ref lg) = thumbnails.lg {
        body.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
        body.extend_from_slice(b"Content-Type: application/octet-stream\r\n");
        body.extend_from_slice(format!("X-Thumbnail-Size: lg\r\n").as_bytes());
        body.extend_from_slice(format!("X-Encryption-IV: {}\r\n\r\n", lg.iv).as_bytes());
        body.extend_from_slice(&lg.data);
        body.extend_from_slice(b"\r\n");
    }

    // End boundary
    body.extend_from_slice(format!("--{}--\r\n", boundary).as_bytes());

    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        format!("multipart/mixed; boundary={}", boundary)
            .parse()
            .unwrap(),
    );

    Ok((headers, Body::from(body)))
}
