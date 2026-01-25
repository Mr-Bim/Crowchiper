use axum::http::HeaderMap;
use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    middleware,
    response::IntoResponse,
    routing::{delete, post},
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use super::error::{ApiError, ResultExt, validate_uuid};
use crate::auth::{ACCESS_COOKIE_NAME, REFRESH_COOKIE_NAME, get_cookie};
use crate::db::{Database, UserRole};
use crate::jwt::JwtConfig;
use crate::rate_limit::{RateLimitConfig, rate_limit_user_create};

#[derive(Clone)]
pub struct UsersState {
    pub db: Database,
    pub jwt: Arc<JwtConfig>,
    pub no_signup: bool,
    pub rate_limit_config: Arc<RateLimitConfig>,
}

pub fn router(state: UsersState) -> Router {
    let delete_router = Router::new()
        .route("/{uuid}", delete(delete_user))
        .with_state(state.clone());

    if state.no_signup {
        delete_router
    } else {
        let create_router = Router::new()
            .route("/", post(create_user))
            .with_state(state.clone())
            .layer(middleware::from_fn_with_state(
                state.rate_limit_config,
                rate_limit_user_create,
            ));

        Router::new().merge(delete_router).merge(create_router)
    }
}

#[derive(Deserialize)]
struct CreateUserRequest {
    username: String,
}

#[derive(Serialize)]
struct CreateUserResponse {
    uuid: String,
    username: String,
}

async fn create_user(
    State(state): State<UsersState>,
    Json(payload): Json<CreateUserRequest>,
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

    // Only allow alphanumeric and underscores
    if !username
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_')
    {
        return Err(ApiError::bad_request(
            "Username can only contain letters, numbers, and underscores",
        ));
    }

    let uuid = uuid::Uuid::new_v4().to_string();

    // Check availability (also cleans up old pending users)
    let available = state
        .db
        .users()
        .is_username_available(username)
        .await
        .db_err("Failed to check username availability")?;

    if !available {
        return Err(ApiError::conflict("Username is already taken"));
    }

    state
        .db
        .users()
        .create(&uuid, username)
        .await
        .db_err("Failed to create user")?;

    Ok((
        StatusCode::CREATED,
        Json(CreateUserResponse {
            uuid,
            username: username.to_string(),
        }),
    ))
}

async fn delete_user(
    State(state): State<UsersState>,
    headers: HeaderMap,
    axum::extract::Path(uuid): axum::extract::Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    validate_uuid(&uuid)?;

    // Find the user by UUID
    let user = state
        .db
        .users()
        .get_by_uuid(&uuid)
        .await
        .db_err("Failed to get user")?
        .ok_or_else(|| ApiError::not_found("User not found"))?;

    // For activated users, require authentication
    if user.activated {
        // Try access token first, then refresh token
        let claims = if let Some(token) = get_cookie(&headers, ACCESS_COOKIE_NAME) {
            state
                .jwt
                .validate_access_token(token)
                .map_err(|_| ApiError::unauthorized("Invalid or expired token"))?
        } else if let Some(token) = get_cookie(&headers, REFRESH_COOKIE_NAME) {
            let refresh_claims = state
                .jwt
                .validate_refresh_token(token)
                .map_err(|_| ApiError::unauthorized("Invalid or expired token"))?;
            // Convert refresh claims to have the same fields we need
            // Check if refresh token is in database (not revoked)
            state
                .db
                .tokens()
                .get_by_jti(&refresh_claims.jti)
                .await
                .map_err(|e| {
                    tracing::error!("Failed to check token: {}", e);
                    ApiError::internal("Unable to check token")
                })?
                .ok_or(ApiError::unauthorized("Invalid or expired token"))?;

            crate::jwt::AccessClaims {
                sub: refresh_claims.sub,
                username: refresh_claims.username,
                role: refresh_claims.role,
                token_type: crate::jwt::TokenType::Access,
                iat: refresh_claims.iat,
                exp: refresh_claims.exp,
                ipaddr: "_".to_owned(),
            }
        } else {
            return Err(ApiError::unauthorized("Authentication required"));
        };

        // Only allow the user themselves or an admin to delete
        let is_self = claims.sub == uuid;
        let is_admin = claims.role == UserRole::Admin;

        if !is_self && !is_admin {
            return Err(ApiError::forbidden("You can only delete your own account"));
        }
    }

    let deleted = state
        .db
        .users()
        .delete(user.id)
        .await
        .db_err("Failed to delete user")?;

    if !deleted {
        return Err(ApiError::not_found("User not found"));
    }

    Ok(StatusCode::NO_CONTENT)
}
