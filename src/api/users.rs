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
use crate::auth::OptionalAuth;
use crate::cli::IpExtractor;
use crate::db::{Database, UserRole};
use crate::impl_has_auth_state;
use crate::jwt::JwtConfig;
use crate::rate_limit::{RateLimitConfig, rate_limit_user_create};

#[derive(Clone)]
pub struct UsersState {
    pub db: Database,
    pub jwt: Arc<JwtConfig>,
    pub secure_cookies: bool,
    pub ip_extractor: Option<IpExtractor>,
    pub no_signup: bool,
    pub rate_limit_config: Arc<RateLimitConfig>,
}

impl_has_auth_state!(UsersState);

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
    OptionalAuth(auth_user): OptionalAuth,
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
        let claims = auth_user
            .ok_or_else(|| ApiError::unauthorized("Authentication required"))?
            .claims;

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
