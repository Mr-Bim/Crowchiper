//! Passkey registration and authentication API endpoints.
//!
//! Registration: POST `/register/start` → challenge → `navigator.credentials.create()` → POST `/register/finish`
//! Login: POST `/login/start` → challenge → `navigator.credentials.get()` → POST `/login/finish` → JWT cookies

use axum::{
    Json, Router,
    extract::{Path, State},
    http::{StatusCode, header::SET_COOKIE},
    middleware,
    response::IntoResponse,
    routing::{delete, post},
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, warn};
use webauthn_rs::prelude::*;

use super::error::{ApiError, ResultExt, validate_uuid};
use crate::auth::{REFRESH_COOKIE_NAME, extract_client_ip, get_cookie};
use crate::db::{AuthChallenge, Database, User};
use crate::jwt::JwtConfig;
use crate::rate_limit::{RateLimitConfig, rate_limit_login_finish, rate_limit_login_start};
use crate::server_config;

#[derive(Clone)]
pub struct PasskeysState {
    pub db: Database,
    pub webauthn: Arc<Webauthn>,
    pub jwt: Arc<JwtConfig>,
}

/// Result of generating auth cookies, includes info needed for token tracking.
struct AuthCookiesResult {
    refresh_cookie: String,
    refresh_jti: String,
    refresh_issued_at: u64,
    refresh_expires_at: u64,
}

impl PasskeysState {
    /// Generate JWT cookies (access + refresh) for the given user.
    /// Returns the cookie strings and refresh token tracking info.
    fn make_refresh_token(&self, user: &User) -> Result<AuthCookiesResult, ApiError> {
        // Generate long-lived refresh token (2 weeks)
        let refresh_result = self
            .jwt
            .generate_refresh_token(&user.uuid, &user.username, user.role)
            .map_err(|e| {
                error!("Failed to generate refresh token: {}", e);
                ApiError::internal("Failed to generate token")
            })?;

        let secure = if server_config::secure_cookies() {
            "; Secure"
        } else {
            ""
        };

        let refresh_cookie = format!(
            "{}={}; HttpOnly; SameSite=Strict; Path=/; Max-Age={}{}",
            REFRESH_COOKIE_NAME, refresh_result.token, refresh_result.duration, secure
        );

        Ok(AuthCookiesResult {
            refresh_cookie,
            refresh_jti: refresh_result.jti,
            refresh_issued_at: refresh_result.issued_at,
            refresh_expires_at: refresh_result.expires_at,
        })
    }

    /// Store a new refresh token in the database for tracking.
    async fn store_refresh_token(
        &self,
        jti: &str,
        user_id: i64,
        ip: Option<&str>,
        issued_at: u64,
        expires_at: u64,
    ) -> Result<(), ApiError> {
        self.db
            .tokens()
            .create(jti, user_id, ip, issued_at, expires_at)
            .await
            .map_err(|e| {
                error!("Failed to store refresh token: {}", e);
                ApiError::internal("Failed to store token")
            })?;
        Ok(())
    }
}

pub fn router(state: PasskeysState, rate_limit_config: Arc<RateLimitConfig>) -> Router {
    // Routes with rate limiting on start endpoints (challenge generation)
    let start_routes = Router::new()
        .route("/register/start", post(register_start))
        .route("/login/start", post(login_start))
        .route("/claim/start", post(claim_start))
        .with_state(state.clone())
        .layer(middleware::from_fn_with_state(
            rate_limit_config.clone(),
            rate_limit_login_start,
        ));

    // Routes with rate limiting on finish endpoints (authentication attempts)
    let finish_routes = Router::new()
        .route("/register/finish", post(register_finish))
        .route("/login/finish", post(login_finish))
        .route("/claim/finish", post(claim_finish))
        .with_state(state.clone())
        .layer(middleware::from_fn_with_state(
            rate_limit_config,
            rate_limit_login_finish,
        ));

    // Routes without rate limiting
    let other_routes = Router::new()
        .route(
            "/login/challenge/{session_id}",
            delete(delete_login_challenge),
        )
        .with_state(state);

    Router::new()
        .merge(start_routes)
        .merge(finish_routes)
        .merge(other_routes)
}

#[derive(Deserialize)]
struct RegisterStartRequest {
    uuid: String,
    /// "passkey" for Google Password Manager passkeys (Android), "security_key" for security keys
    #[serde(default)]
    authenticator_type: AuthenticatorType,
}

#[derive(Deserialize, Default, Clone, Copy)]
#[serde(rename_all = "snake_case")]
enum AuthenticatorType {
    /// Google Passkey in Google Password Manager (Android with GMS Core)
    Passkey,
    /// Security key or device screen unlock (default, works everywhere)
    #[default]
    SecurityKey,
}

#[derive(Deserialize)]
struct RegisterFinishRequest {
    uuid: String,
    credential: RegisterPublicKeyCredential,
}

#[derive(Deserialize)]
struct LoginStartRequest {
    username: Option<String>,
}

#[derive(Serialize)]
struct LoginStartResponse {
    session_id: String,
    #[serde(flatten)]
    options: RequestChallengeResponse,
}

#[derive(Deserialize)]
struct LoginFinishRequest {
    session_id: String,
    credential: PublicKeyCredential,
}

async fn register_start(
    State(state): State<PasskeysState>,
    Json(payload): Json<RegisterStartRequest>,
) -> Result<impl IntoResponse, ApiError> {
    validate_uuid(&payload.uuid)?;

    let user = state
        .db
        .users()
        .get_by_uuid(&payload.uuid)
        .await
        .db_err("Failed to get user")?
        .ok_or_else(|| ApiError::not_found("User not found"))?;

    if user.activated {
        return Err(ApiError::bad_request("User already has a passkey"));
    }

    let existing: Vec<Passkey> = state
        .db
        .passkeys()
        .get_by_user_id(user.id)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|p| p.passkey)
        .collect();

    let user_id = Uuid::parse_str(&user.uuid).unwrap_or_else(|_| Uuid::new_v4());
    let exclude =
        (!existing.is_empty()).then(|| existing.iter().map(|p| p.cred_id().clone()).collect());

    let (ccr, reg_state) = match payload.authenticator_type {
        AuthenticatorType::Passkey => state
            .webauthn
            .start_google_passkey_in_google_password_manager_only_registration(
                user_id,
                &user.username,
                &user.username,
                exclude,
            )
            .webauthn_err("Failed to start registration")?,
        AuthenticatorType::SecurityKey => state
            .webauthn
            .start_passkey_registration(user_id, &user.username, &user.username, exclude)
            .webauthn_err("Failed to start registration")?,
    };

    state
        .db
        .challenges()
        .store(&payload.uuid, &reg_state)
        .await
        .db_err("Failed to store challenge")?;

    Ok((StatusCode::OK, Json(ccr)))
}

#[derive(Serialize)]
struct RegisterFinishResponse {
    passkey_id: i64,
}

async fn register_finish(
    State(state): State<PasskeysState>,
    request: axum::extract::Request,
) -> Result<impl IntoResponse, ApiError> {
    let (parts, body) = request.into_parts();
    let Json(payload): Json<RegisterFinishRequest> = Json::from_bytes(
        &axum::body::to_bytes(body, 1024 * 1024)
            .await
            .map_err(|_| ApiError::bad_request("Invalid request body"))?,
    )
    .map_err(|_| ApiError::bad_request("Invalid JSON"))?;

    validate_uuid(&payload.uuid)?;

    let reg_state = state
        .db
        .challenges()
        .take(&payload.uuid)
        .await
        .db_err("Failed to get challenge")?
        .ok_or_else(|| ApiError::bad_request("No pending registration or challenge expired"))?;

    let passkey = state
        .webauthn
        .finish_passkey_registration(&payload.credential, &reg_state)
        .map_err(|e| {
            warn!("Registration failed: {}", e);
            ApiError::bad_request("Registration failed")
        })?;

    let user = state
        .db
        .users()
        .get_by_uuid(&payload.uuid)
        .await
        .db_err("Failed to get user")?
        .ok_or_else(|| ApiError::not_found("User not found"))?;

    let passkey_id = state
        .db
        .passkeys()
        .add(user.id, &passkey)
        .await
        .db_err("Failed to store passkey")?;
    state
        .db
        .users()
        .activate(user.id)
        .await
        .db_err("Failed to activate user")?;

    let refresh_token = state.make_refresh_token(&user)?;

    // Store refresh token for tracking
    let ip = extract_client_ip(&parts, server_config::ip_extractor().as_ref()).ok();
    state
        .store_refresh_token(
            &refresh_token.refresh_jti,
            user.id,
            ip.as_deref(),
            refresh_token.refresh_issued_at,
            refresh_token.refresh_expires_at,
        )
        .await?;

    Ok((
        StatusCode::OK,
        [(SET_COOKIE, refresh_token.refresh_cookie)],
        Json(RegisterFinishResponse { passkey_id }),
    ))
}

async fn login_start(
    State(state): State<PasskeysState>,
    Json(payload): Json<LoginStartRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let user_passkeys = get_user_passkeys(&state.db, payload.username.as_deref()).await;

    let (rcr, challenge) = if let Some(passkeys) = user_passkeys {
        let (rcr, auth_state) = state
            .webauthn
            .start_passkey_authentication(&passkeys)
            .webauthn_err("Failed to start authentication")?;
        (rcr, AuthChallenge::Passkey(auth_state))
    } else {
        let (rcr, auth_state) = state
            .webauthn
            .start_discoverable_authentication()
            .webauthn_err("Failed to start discoverable authentication")?;
        (rcr, AuthChallenge::Discoverable(auth_state))
    };

    let session_id = uuid::Uuid::new_v4().to_string();
    state
        .db
        .login_challenges()
        .store(&session_id, &challenge)
        .await
        .db_err("Failed to store challenge")?;

    Ok((
        StatusCode::OK,
        Json(LoginStartResponse {
            session_id,
            options: rcr,
        }),
    ))
}

#[derive(Serialize)]
struct LoginFinishResponse {
    passkey_id: i64,
    /// Whether the user account is activated
    activated: bool,
    /// Whether encryption setup is complete
    encryption_setup_finished: bool,
}

async fn login_finish(
    State(state): State<PasskeysState>,
    request: axum::extract::Request,
) -> Result<impl IntoResponse, ApiError> {
    let (parts, body) = request.into_parts();
    let Json(payload): Json<LoginFinishRequest> = Json::from_bytes(
        &axum::body::to_bytes(body, 1024 * 1024)
            .await
            .map_err(|_| ApiError::bad_request("Invalid request body"))?,
    )
    .map_err(|_| ApiError::bad_request("Invalid JSON"))?;

    let challenge = state
        .db
        .login_challenges()
        .take(&payload.session_id)
        .await
        .db_err("Failed to get challenge")?
        .ok_or_else(|| ApiError::bad_request("No pending authentication or challenge expired"))?;

    let result = match challenge {
        AuthChallenge::Passkey(auth_state) => {
            finish_passkey_auth(&state, &payload.credential, &auth_state).await?
        }
        AuthChallenge::Discoverable(auth_state) => {
            finish_discoverable_auth(&state, &payload.credential, auth_state).await?
        }
    };

    if result.auth_result.needs_update() {
        if let Err(e) = update_passkey_counter(&state.db, &result.auth_result).await {
            error!("Failed to update passkey counter: {}", e);
        }
    }

    // Check if encryption setup is complete (has PRF salt)
    let encryption_setup_finished = state
        .db
        .encryption_settings()
        .get(result.user.id)
        .await
        .db_err("Failed to get encryption settings")?
        .is_some();

    let response = Json(LoginFinishResponse {
        passkey_id: result.passkey_id,
        activated: result.user.activated,
        encryption_setup_finished,
    });
    if !result.user.activated {
        return Ok((StatusCode::OK, response).into_response());
    }

    // Only generate JWT if user is activated
    let ip = extract_client_ip(&parts, server_config::ip_extractor().as_ref()).ok();

    // Revoke existing refresh token if present, then always issue a new one.
    // This invalidates any stolen copies of the old token.
    if let Some(refresh_token_str) = get_cookie(&parts.headers, REFRESH_COOKIE_NAME) {
        if let Ok(claims) = state.jwt.validate_refresh_token(refresh_token_str) {
            let _ = state.db.tokens().delete_by_jti(&claims.jti).await;
        }
    }

    let refresh_token = state.make_refresh_token(&result.user)?;
    state
        .store_refresh_token(
            &refresh_token.refresh_jti,
            result.user.id,
            ip.as_deref(),
            refresh_token.refresh_issued_at,
            refresh_token.refresh_expires_at,
        )
        .await?;
    Ok((
        StatusCode::OK,
        [(SET_COOKIE, refresh_token.refresh_cookie)],
        response,
    )
        .into_response())
}

async fn delete_login_challenge(
    State(state): State<PasskeysState>,
    Path(session_id): Path<String>,
) -> impl IntoResponse {
    let _ = state.db.login_challenges().delete(&session_id).await;
    StatusCode::NO_CONTENT
}

/// Start the claim flow for users who have a passkey but aren't activated.
/// This uses discoverable authentication so the user can authenticate with their existing passkey.
async fn claim_start(State(state): State<PasskeysState>) -> Result<impl IntoResponse, ApiError> {
    let (rcr, auth_state) = state
        .webauthn
        .start_discoverable_authentication()
        .webauthn_err("Failed to start authentication")?;

    let session_id = uuid::Uuid::new_v4().to_string();
    let challenge = AuthChallenge::Discoverable(auth_state);
    state
        .db
        .login_challenges()
        .store(&session_id, &challenge)
        .await
        .db_err("Failed to store challenge")?;

    Ok((
        StatusCode::OK,
        Json(LoginStartResponse {
            session_id,
            options: rcr,
        }),
    ))
}

/// Finish the claim flow: authenticate with passkey and activate the user.
async fn claim_finish(
    State(state): State<PasskeysState>,
    request: axum::extract::Request,
) -> Result<impl IntoResponse, ApiError> {
    let (parts, body) = request.into_parts();
    let Json(payload): Json<LoginFinishRequest> = Json::from_bytes(
        &axum::body::to_bytes(body, 1024 * 1024)
            .await
            .map_err(|_| ApiError::bad_request("Invalid request body"))?,
    )
    .map_err(|_| ApiError::bad_request("Invalid JSON"))?;

    let challenge = state
        .db
        .login_challenges()
        .take(&payload.session_id)
        .await
        .db_err("Failed to get challenge")?
        .ok_or_else(|| ApiError::bad_request("No pending authentication or challenge expired"))?;

    let auth_state = match challenge {
        AuthChallenge::Discoverable(state) => state,
        AuthChallenge::Passkey(_) => {
            return Err(ApiError::bad_request("Invalid challenge type for claim"));
        }
    };

    let result = finish_discoverable_auth(&state, &payload.credential, auth_state).await?;

    // Activate the user if not already activated
    if !result.user.activated {
        state
            .db
            .users()
            .activate(result.user.id)
            .await
            .db_err("Failed to activate user")?;
    }

    if result.auth_result.needs_update() {
        if let Err(e) = update_passkey_counter(&state.db, &result.auth_result).await {
            error!("Failed to update passkey counter: {}", e);
        }
    }

    // Check if encryption setup is complete
    let encryption_setup_finished = state
        .db
        .encryption_settings()
        .get(result.user.id)
        .await
        .db_err("Failed to get encryption settings")?
        .map(|s| s.setup_done)
        .unwrap_or(false);

    let refresh_token = state.make_refresh_token(&result.user)?;

    // Store refresh token for tracking
    let ip = extract_client_ip(&parts, server_config::ip_extractor().as_ref()).ok();
    state
        .store_refresh_token(
            &refresh_token.refresh_jti,
            result.user.id,
            ip.as_deref(),
            refresh_token.refresh_issued_at,
            refresh_token.refresh_expires_at,
        )
        .await?;

    Ok((
        StatusCode::OK,
        [(SET_COOKIE, refresh_token.refresh_cookie)],
        Json(LoginFinishResponse {
            passkey_id: result.passkey_id,
            activated: true, // claim_finish always activates the user
            encryption_setup_finished,
        }),
    ))
}

async fn get_user_passkeys(db: &Database, username: Option<&str>) -> Option<Vec<Passkey>> {
    let username = username?.trim();
    if username.is_empty() {
        return None;
    }
    let user = db.users().get_by_username(username).await.ok()??;
    // Note: We allow passkey auth for unactivated users too - they need to be able
    // to authenticate to reclaim their account. The activation check is done in
    // login_finish, which returns 403 for unactivated users.
    let passkeys = db.passkeys().get_by_user_id(user.id).await.ok()?;
    let keys: Vec<Passkey> = passkeys.into_iter().map(|p| p.passkey).collect();
    (!keys.is_empty()).then_some(keys)
}

/// Result of authentication including the passkey ID used
struct AuthResult {
    auth_result: AuthenticationResult,
    user: User,
    passkey_id: i64,
}

async fn finish_passkey_auth(
    state: &PasskeysState,
    credential: &PublicKeyCredential,
    auth_state: &PasskeyAuthentication,
) -> Result<AuthResult, ApiError> {
    let auth_result = state
        .webauthn
        .finish_passkey_authentication(credential, auth_state)
        .map_err(|e| {
            error!("Passkey authentication failed: {}", e);
            ApiError::unauthorized("Authentication failed")
        })?;

    let stored = state
        .db
        .passkeys()
        .get_by_credential_id(auth_result.cred_id().as_ref())
        .await
        .db_err("Failed to get passkey")?
        .ok_or_else(|| ApiError::unauthorized("Passkey not found"))?;

    let user = state
        .db
        .users()
        .get_by_id(stored.user_id)
        .await
        .db_err("Failed to get user")?
        .ok_or_else(|| ApiError::unauthorized("User not found"))?;

    Ok(AuthResult {
        auth_result,
        user,
        passkey_id: stored.id,
    })
}

async fn finish_discoverable_auth(
    state: &PasskeysState,
    credential: &PublicKeyCredential,
    auth_state: DiscoverableAuthentication,
) -> Result<AuthResult, ApiError> {
    let user_handle = credential
        .response
        .user_handle
        .as_ref()
        .ok_or_else(|| ApiError::bad_request("Credential missing user handle"))?;

    let user_uuid = uuid::Uuid::from_slice(user_handle.as_ref())
        .map_err(|_| ApiError::bad_request("Invalid user handle format"))?
        .to_string();

    let user = state
        .db
        .users()
        .get_by_uuid(&user_uuid)
        .await
        .db_err("Failed to get user")?
        .ok_or_else(|| ApiError::unauthorized("User not found"))?;

    let keys: Vec<DiscoverableKey> = state
        .db
        .passkeys()
        .get_by_user_id(user.id)
        .await
        .db_err("Failed to get passkeys")?
        .into_iter()
        .map(|p| p.passkey.into())
        .collect();

    if keys.is_empty() {
        return Err(ApiError::unauthorized("User has no passkeys"));
    }

    let auth_result = state
        .webauthn
        .finish_discoverable_authentication(credential, auth_state, &keys)
        .map_err(|e| {
            error!("Discoverable authentication failed: {}", e);
            ApiError::unauthorized("Authentication failed")
        })?;

    // Look up the passkey ID from the credential ID used
    let stored = state
        .db
        .passkeys()
        .get_by_credential_id(auth_result.cred_id().as_ref())
        .await
        .db_err("Failed to get passkey")?
        .ok_or_else(|| ApiError::unauthorized("Passkey not found"))?;

    Ok(AuthResult {
        auth_result,
        user,
        passkey_id: stored.id,
    })
}

async fn update_passkey_counter(
    db: &Database,
    auth_result: &AuthenticationResult,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let stored = db
        .passkeys()
        .get_by_credential_id(auth_result.cred_id().as_ref())
        .await?
        .ok_or("Passkey not found")?;
    let mut updated = stored.passkey.clone();
    updated.update_credential(auth_result);
    db.passkeys().update(&updated).await?;
    Ok(())
}
