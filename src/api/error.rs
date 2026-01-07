//! Shared error handling for API endpoints.

use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Serialize;
use tracing::error;

/// Extension trait for concise error mapping on Results.
pub trait ResultExt<T> {
    fn db_err(self, msg: &str) -> Result<T, ApiError>;
    fn webauthn_err(self, msg: &str) -> Result<T, ApiError>;
}

impl<T, E: std::fmt::Display> ResultExt<T> for Result<T, E> {
    fn db_err(self, msg: &str) -> Result<T, ApiError> {
        self.map_err(|e| ApiError::db_error(msg, e))
    }
    fn webauthn_err(self, msg: &str) -> Result<T, ApiError> {
        self.map_err(|e| ApiError::webauthn_error(msg, e))
    }
}

/// API error type with automatic response conversion.
pub enum ApiError {
    BadRequest(String),
    Forbidden(String),
    NotFound(String),
    Unauthorized(String),
    Conflict(String),
    Internal(String),
}

impl ApiError {
    pub fn bad_request(msg: impl Into<String>) -> Self {
        Self::BadRequest(msg.into())
    }

    pub fn forbidden(msg: impl Into<String>) -> Self {
        Self::Forbidden(msg.into())
    }

    pub fn not_found(msg: impl Into<String>) -> Self {
        Self::NotFound(msg.into())
    }

    pub fn unauthorized(msg: impl Into<String>) -> Self {
        Self::Unauthorized(msg.into())
    }

    pub fn conflict(msg: impl Into<String>) -> Self {
        Self::Conflict(msg.into())
    }

    pub fn internal(msg: impl Into<String>) -> Self {
        Self::Internal(msg.into())
    }

    pub fn db_error(context: &str, e: impl std::fmt::Display) -> Self {
        error!("{}: {}", context, e);
        Self::Internal("Database error".into())
    }

    pub fn webauthn_error(context: &str, e: impl std::fmt::Display) -> Self {
        error!("{}: {}", context, e);
        Self::Internal(context.into())
    }
}

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            ApiError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg),
            ApiError::Forbidden(msg) => (StatusCode::FORBIDDEN, msg),
            ApiError::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
            ApiError::Unauthorized(msg) => (StatusCode::UNAUTHORIZED, msg),
            ApiError::Conflict(msg) => (StatusCode::CONFLICT, msg),
            ApiError::Internal(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
        };
        (status, Json(ErrorResponse { error: message })).into_response()
    }
}

/// Validate a UUID string format.
pub fn validate_uuid(uuid: &str) -> Result<(), ApiError> {
    if uuid.is_empty() {
        return Err(ApiError::bad_request("UUID cannot be empty"));
    }
    if uuid.len() > 36 {
        return Err(ApiError::bad_request("UUID is too long"));
    }
    if uuid::Uuid::parse_str(uuid).is_err() {
        return Err(ApiError::bad_request("Invalid UUID format"));
    }
    Ok(())
}
