//! Test-mode only API endpoints.
//!
//! These endpoints are only available when compiled with the `test-mode` feature.
//! They provide functionality needed for e2e testing that shouldn't be exposed in production.

use axum::{Json, Router, extract::State, http::StatusCode, response::IntoResponse, routing::post};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use super::error::{ApiError, ResultExt};
use crate::db::{Database, UserRole};
use crate::jwt::JwtConfig;

#[derive(Clone)]
pub struct TestState {
    pub db: Database,
    pub jwt: Arc<JwtConfig>,
}

pub fn router(state: TestState) -> Router {
    Router::new()
        .route("/admin", post(create_admin_user))
        .route("/token", post(store_token))
        .route("/generate-tokens", post(generate_tokens))
        .with_state(state)
}

#[derive(Deserialize)]
struct CreateAdminRequest {
    username: String,
}

#[derive(Serialize)]
struct CreateAdminResponse {
    uuid: String,
    username: String,
}

/// Create an admin user for testing claim flows.
async fn create_admin_user(
    State(state): State<TestState>,
    Json(payload): Json<CreateAdminRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let username = payload.username.trim();

    if username.is_empty() {
        return Err(ApiError::bad_request("Username cannot be empty"));
    }

    if username.len() > 32 {
        return Err(ApiError::bad_request(
            "Username cannot be longer than 32 characters",
        ));
    }

    let uuid = uuid::Uuid::new_v4().to_string();

    state
        .db
        .users()
        .create_admin(&uuid, username)
        .await
        .db_err("Failed to create admin user")?;

    Ok((
        StatusCode::CREATED,
        Json(CreateAdminResponse {
            uuid,
            username: username.to_string(),
        }),
    ))
}

#[derive(Deserialize)]
struct StoreTokenRequest {
    jti: String,
    user_uuid: String,
    username: String,
    issued_at: u64,
    expires_at: u64,
}

#[derive(Serialize)]
struct StoreTokenResponse {
    success: bool,
}

/// Store a token in the database for e2e testing.
/// This allows tests to manually create JWT tokens and register them.
async fn store_token(
    State(state): State<TestState>,
    Json(payload): Json<StoreTokenRequest>,
) -> Result<impl IntoResponse, ApiError> {
    // First, ensure the user exists and is activated
    let user = state
        .db
        .users()
        .get_by_uuid(&payload.user_uuid)
        .await
        .db_err("Failed to get user")?;

    let user_id = if let Some(user) = user {
        user.id
    } else {
        // Create the user if it doesn't exist
        let id = state
            .db
            .users()
            .create(&payload.user_uuid, &payload.username)
            .await
            .db_err("Failed to create user")?;
        state
            .db
            .users()
            .activate(id)
            .await
            .db_err("Failed to activate user")?;
        state
            .db
            .encryption_settings()
            .mark_setup_done(id)
            .await
            .db_err("Failed to mark encryption setup done")?;
        id
    };

    // Store the token
    state
        .db
        .tokens()
        .create(
            &payload.jti,
            user_id,
            None,
            payload.issued_at,
            payload.expires_at,
        )
        .await
        .db_err("Failed to store token")?;

    Ok((StatusCode::OK, Json(StoreTokenResponse { success: true })))
}

#[derive(Deserialize)]
struct GenerateTokensRequest {
    user_uuid: String,
    username: String,
    #[serde(default)]
    role: Option<String>,
    #[serde(default)]
    ip_addr: Option<String>,
    /// If true, generate an expired access token
    #[serde(default)]
    expired_access: bool,
    /// If true, also store the refresh token in the database
    #[serde(default)]
    store_refresh: bool,
}

#[derive(Serialize)]
struct GenerateTokensResponse {
    access_token: String,
    refresh_token: String,
    refresh_jti: String,
    issued_at: u64,
    expires_at: u64,
}

/// Generate access and refresh tokens for e2e testing.
/// This allows tests to create valid JWT tokens without needing a JS JWT library.
async fn generate_tokens(
    State(state): State<TestState>,
    Json(payload): Json<GenerateTokensRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let role = match payload.role.as_deref() {
        Some("admin") => UserRole::Admin,
        _ => UserRole::User,
    };
    let ip_addr = payload.ip_addr.as_deref().unwrap_or("127.0.0.1");

    // Generate refresh token
    let refresh_result = state
        .jwt
        .generate_refresh_token(&payload.user_uuid, &payload.username, role)
        .map_err(|e| ApiError::internal(format!("Failed to generate refresh token: {}", e)))?;

    // Generate access token (possibly expired)
    let access_token = if payload.expired_access {
        // Create an expired access token manually
        generate_expired_access_token(&payload.user_uuid, &payload.username, role, ip_addr)
            .map_err(|e| ApiError::internal(format!("Failed to generate expired token: {}", e)))?
    } else {
        state
            .jwt
            .generate_access_token(&payload.user_uuid, &payload.username, role, ip_addr)
            .map_err(|e| ApiError::internal(format!("Failed to generate access token: {}", e)))?
            .token
    };

    // Optionally store refresh token in database
    if payload.store_refresh {
        // First, ensure the user exists and is activated
        let user = state
            .db
            .users()
            .get_by_uuid(&payload.user_uuid)
            .await
            .db_err("Failed to get user")?;

        let user_id = if let Some(user) = user {
            user.id
        } else {
            // Create the user if it doesn't exist
            let id = state
                .db
                .users()
                .create(&payload.user_uuid, &payload.username)
                .await
                .db_err("Failed to create user")?;
            state
                .db
                .users()
                .activate(id)
                .await
                .db_err("Failed to activate user")?;
            state
                .db
                .encryption_settings()
                .mark_setup_done(id)
                .await
                .db_err("Failed to mark encryption setup done")?;
            id
        };

        state
            .db
            .tokens()
            .create(
                &refresh_result.jti,
                user_id,
                Some(ip_addr),
                refresh_result.issued_at,
                refresh_result.expires_at,
            )
            .await
            .db_err("Failed to store refresh token")?;
    }

    Ok((
        StatusCode::OK,
        Json(GenerateTokensResponse {
            access_token,
            refresh_token: refresh_result.token,
            refresh_jti: refresh_result.jti,
            issued_at: refresh_result.issued_at,
            expires_at: refresh_result.expires_at,
        }),
    ))
}

/// Generate an expired access token for testing token refresh flows.
fn generate_expired_access_token(
    user_uuid: &str,
    username: &str,
    role: UserRole,
    ip_addr: &str,
) -> Result<String, String> {
    use jsonwebtoken::{EncodingKey, Header};
    use std::time::{SystemTime, UNIX_EPOCH};

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "Time error")?
        .as_secs();

    let claims = crate::jwt::AccessClaims {
        sub: user_uuid.to_string(),
        username: username.to_string(),
        role,
        token_type: crate::jwt::TokenType::Access,
        iat: now - 400,
        exp: now - 100, // Expired
        ipaddr: ip_addr.to_string(),
    };

    // Use the same secret that's used in test mode
    let secret = b"test-jwt-secret-for-playwright-testing-minimum-32-chars";
    let encoding_key = EncodingKey::from_secret(secret);

    jsonwebtoken::encode(&Header::default(), &claims, &encoding_key)
        .map_err(|e| format!("Encoding error: {}", e))
}
