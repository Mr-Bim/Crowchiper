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
use crate::auth::{ActivatedApiAuth, HasAuthState};
use crate::cli::IpExtractor;
use crate::db::{Database, attachments::CreateAttachmentInput};
use crate::jwt::JwtConfig;

/// Encryption version 0 means unencrypted data
const UNENCRYPTED_VERSION: i32 = 0;

/// State for attachments endpoints.
#[derive(Clone)]
pub struct AttachmentsState {
    pub db: Database,
    pub jwt: Arc<JwtConfig>,
    pub secure_cookies: bool,
    pub ip_extractor: Option<IpExtractor>,
}

impl HasAuthState for AttachmentsState {
    fn jwt(&self) -> &JwtConfig {
        &self.jwt
    }
    fn db(&self) -> &Database {
        &self.db
    }
    fn secure_cookies(&self) -> bool {
        self.secure_cookies
    }
    fn ip_extractor(&self) -> Option<&IpExtractor> {
        self.ip_extractor.as_ref()
    }
}

pub fn router(state: AttachmentsState) -> Router {
    Router::new()
        .route("/", post(upload_attachment))
        .route("/{uuid}", get(get_attachment))
        .route("/{uuid}/thumbnails", get(get_thumbnails))
        .route("/{uuid}/thumbnail/{size}", get(get_thumbnail))
        // 20MB limit: 10MB image + thumbnails + overhead
        .layer(DefaultBodyLimit::max(20 * 1024 * 1024))
        .with_state(state)
}

// --- Response types ---

#[derive(Serialize)]
struct UploadResponse {
    uuid: String,
}

// --- Handlers ---

/// Upload an attachment using multipart form data.
/// Supports both encrypted (encryption_version > 0) and unencrypted (encryption_version = 0) uploads.
///
/// Expected fields:
/// - `image`: Binary image data (encrypted or raw)
/// - `image_iv`: IV for image (base64url string, empty for unencrypted)
/// - `thumb_sm`: Binary small thumbnail (200px)
/// - `thumb_sm_iv`: IV for small thumbnail (empty for unencrypted)
/// - `thumb_md`: Binary medium thumbnail (400px) - optional
/// - `thumb_md_iv`: IV for medium thumbnail - optional
/// - `thumb_lg`: Binary large thumbnail (800px) - optional
/// - `thumb_lg_iv`: IV for large thumbnail - optional
/// - `encryption_version`: Version number (0 = unencrypted, >0 = encrypted)
///
/// If user has encryption enabled, encryption_version must be > 0.
/// If user does not have encryption enabled, encryption_version must be 0.
async fn upload_attachment(
    State(state): State<AttachmentsState>,
    ActivatedApiAuth(user): ActivatedApiAuth,
    mut multipart: Multipart,
) -> Result<impl IntoResponse, ApiError> {
    let mut image_data: Option<Vec<u8>> = None;
    let mut image_iv: Option<String> = None;
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
                image_data = Some(data.to_vec());
            }
            "image_iv" => {
                let text = field
                    .text()
                    .await
                    .map_err(|_| ApiError::bad_request("Failed to read image_iv"))?;
                image_iv = Some(text);
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

    let image_data = image_data.ok_or_else(|| ApiError::bad_request("Missing image field"))?;
    let image_iv = image_iv.ok_or_else(|| ApiError::bad_request("Missing image_iv field"))?;
    let thumb_sm = thumb_sm.ok_or_else(|| ApiError::bad_request("Missing thumb_sm field"))?;
    let thumb_sm_iv =
        thumb_sm_iv.ok_or_else(|| ApiError::bad_request("Missing thumb_sm_iv field"))?;
    let encryption_version = encryption_version
        .ok_or_else(|| ApiError::bad_request("Missing encryption_version field"))?;

    // Check user's encryption settings and validate encryption_version
    let encryption_settings = state
        .db
        .encryption_settings()
        .get(user.user_id)
        .await
        .db_err("Failed to get encryption settings")?;

    let user_has_encryption = encryption_settings
        .map(|s| s.encryption_enabled)
        .unwrap_or(false);

    if user_has_encryption && encryption_version == UNENCRYPTED_VERSION {
        return Err(ApiError::bad_request(
            "Encryption is enabled but unencrypted data was submitted",
        ));
    }

    if !user_has_encryption && encryption_version != UNENCRYPTED_VERSION {
        return Err(ApiError::bad_request(
            "Encryption is not enabled but encrypted data was submitted",
        ));
    }

    // Limit image size to 10MB
    if image_data.len() > 10 * 1024 * 1024 {
        return Err(ApiError::bad_request("Image too large (max 10MB)"));
    }

    // Limit thumbnail sizes (frontend targets: sm=100KB, md=200KB, lg=400KB + encryption overhead)
    if thumb_sm.len() > 150 * 1024 {
        return Err(ApiError::bad_request(
            "Small thumbnail too large (max 150KB)",
        ));
    }
    if let Some(ref data) = thumb_md {
        if data.len() > 250 * 1024 {
            return Err(ApiError::bad_request(
                "Medium thumbnail too large (max 250KB)",
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

    // Convert empty IVs to None for unencrypted uploads
    let image_iv_opt = if image_iv.is_empty() {
        None
    } else {
        Some(image_iv)
    };
    let thumb_sm_iv_opt = if thumb_sm_iv.is_empty() {
        None
    } else {
        Some(thumb_sm_iv)
    };

    let input = CreateAttachmentInput {
        user_id: user.user_id,
        image_data: &image_data,
        image_iv: image_iv_opt.as_deref(),
        thumb_sm: &thumb_sm,
        thumb_sm_iv: thumb_sm_iv_opt.as_deref(),
        thumb_md: thumb_md.as_ref().zip(thumb_md_iv.as_ref()).map(|(d, iv)| {
            (
                d.as_slice(),
                if iv.is_empty() {
                    None
                } else {
                    Some(iv.as_str())
                },
            )
        }),
        thumb_lg: thumb_lg.as_ref().zip(thumb_lg_iv.as_ref()).map(|(d, iv)| {
            (
                d.as_slice(),
                if iv.is_empty() {
                    None
                } else {
                    Some(iv.as_str())
                },
            )
        }),
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

/// Get an attachment as binary stream.
/// IV is returned in the `X-Encryption-IV` header (empty string if unencrypted).
async fn get_attachment(
    State(state): State<AttachmentsState>,
    ActivatedApiAuth(user): ActivatedApiAuth,
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
    headers.insert(header::CONTENT_DISPOSITION, "attachment".parse().unwrap());

    // Empty string for unencrypted attachments
    let iv = attachment.image_iv.unwrap_or_default();
    headers.insert("X-Encryption-IV", iv.parse().unwrap());

    Ok((headers, Body::from(attachment.image_data)))
}

/// Get a single thumbnail by size as binary stream.
/// IV is returned in the `X-Encryption-IV` header (empty string if unencrypted).
/// Size must be "sm", "md", or "lg".
async fn get_thumbnail(
    State(state): State<AttachmentsState>,
    ActivatedApiAuth(user): ActivatedApiAuth,
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
    headers.insert(header::CONTENT_DISPOSITION, "attachment".parse().unwrap());

    // Empty string for unencrypted attachments
    let iv = thumbnail.iv.unwrap_or_default();
    headers.insert("X-Encryption-IV", iv.parse().unwrap());

    Ok((headers, Body::from(thumbnail.data)))
}

/// Get all thumbnails as a multipart response.
/// Each part has `X-Thumbnail-Size` header (sm, md, lg) and `X-Encryption-IV` header (empty if unencrypted).
async fn get_thumbnails(
    State(state): State<AttachmentsState>,
    ActivatedApiAuth(user): ActivatedApiAuth,
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
    let sm_iv = thumbnails.sm.iv.as_deref().unwrap_or("");
    body.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
    body.extend_from_slice(b"Content-Type: application/octet-stream\r\n");
    body.extend_from_slice(b"X-Thumbnail-Size: sm\r\n");
    body.extend_from_slice(format!("X-Encryption-IV: {}\r\n\r\n", sm_iv).as_bytes());
    body.extend_from_slice(&thumbnails.sm.data);
    body.extend_from_slice(b"\r\n");

    // Medium thumbnail (optional)
    if let Some(ref md) = thumbnails.md {
        let md_iv = md.iv.as_deref().unwrap_or("");
        body.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
        body.extend_from_slice(b"Content-Type: application/octet-stream\r\n");
        body.extend_from_slice(b"X-Thumbnail-Size: md\r\n");
        body.extend_from_slice(format!("X-Encryption-IV: {}\r\n\r\n", md_iv).as_bytes());
        body.extend_from_slice(&md.data);
        body.extend_from_slice(b"\r\n");
    }

    // Large thumbnail (optional)
    if let Some(ref lg) = thumbnails.lg {
        let lg_iv = lg.iv.as_deref().unwrap_or("");
        body.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
        body.extend_from_slice(b"Content-Type: application/octet-stream\r\n");
        body.extend_from_slice(b"X-Thumbnail-Size: lg\r\n");
        body.extend_from_slice(format!("X-Encryption-IV: {}\r\n\r\n", lg_iv).as_bytes());
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
