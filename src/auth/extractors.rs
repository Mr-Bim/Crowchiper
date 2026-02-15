//! Axum extractors for authentication with role-based access control.
//!
//! # Extractors
//!
//! - `Auth<R>` - Requires activated account, role checked via `R: RoleConstraint`
//! - `AuthWithSession<R>` - Like `Auth<R>` but includes refresh token JTI
//! - `OptionalAuth` - Returns `Option<AuthenticatedUser>`, never fails
//! - `ProtectedAsset<R>` - For asset endpoints, redirects to login on failure
//!
//! # Examples
//!
//! ```ignore
//! async fn handler(auth: Auth<AnyRole>) { }
//! async fn admin_handler(auth: Auth<AdminOnly>) { }
//! async fn list_sessions(auth: AuthWithSession<AnyRole>) { }
//! ```

use std::cell::RefCell;
use std::marker::PhantomData;
use std::ops::Deref;

use axum::{
    extract::FromRequestParts,
    http::{header::SET_COOKIE, request::Parts},
    middleware::Next,
    response::Response,
};

use super::cookie::{ACCESS_COOKIE_NAME, REFRESH_COOKIE_NAME, get_cookie};
use super::errors::{ApiAuthError, AssetAuthError, AuthErrorKind};
use super::ip::extract_client_ip;
use super::state::{HasAssetAuthBackend, HasAuthBackend};
use super::types::{ActivatedAuthenticatedUser, AuthenticatedUser, AuthenticatedUserWithSession};
use crate::db::UserRole;
use crate::plugin::{Hook, ServerHook};
use crate::server_config;

tokio::task_local! {
    pub static NEW_ACCESS_TOKEN_COOKIE: RefCell<Option<String>>;
}

/// Middleware to attach refreshed access token cookie to responses.
pub async fn add_access_token_cookie(request: axum::extract::Request, next: Next) -> Response {
    NEW_ACCESS_TOKEN_COOKIE
        .scope(RefCell::new(None), async {
            let mut response = next.run(request).await;

            if let Some(cookie) = NEW_ACCESS_TOKEN_COOKIE.with(|cell| cell.borrow_mut().take()) {
                if let Ok(value) = cookie.parse() {
                    response.headers_mut().append(SET_COOKIE, value);
                }
            }

            response
        })
        .await
}

/// Trait for role-based access control.
pub trait RoleConstraint: Send + Sync + 'static {
    fn check(role: UserRole) -> bool;
}

/// Allows any authenticated user regardless of role.
pub struct AnyRole;

impl RoleConstraint for AnyRole {
    fn check(_role: UserRole) -> bool {
        true
    }
}

/// Requires the user to have admin role.
pub struct AdminOnly;

impl RoleConstraint for AdminOnly {
    fn check(role: UserRole) -> bool {
        role == UserRole::Admin
    }
}

/// Core authentication logic shared between all extractors.
///
/// When the access token is valid, returns immediately without touching the
/// refresh token. When the access token is missing/invalid/IP-mismatched,
/// falls back to the refresh token and populates `user_id` and `refresh_jti`.
async fn authenticate_request<S>(
    parts: &Parts,
    state: &S,
) -> Result<AuthenticatedUser, AuthErrorKind>
where
    S: HasAuthBackend + Send + Sync,
{
    let client_ip = extract_client_ip(parts, server_config::ip_extractor().as_ref())
        .map_err(|_| AuthErrorKind::NotAuthenticated)?;

    // Fast path: valid access token with matching IP
    if let Some(access_token) = get_cookie(&parts.headers, ACCESS_COOKIE_NAME) {
        if let Ok(claims) = state.jwt().validate_access_token(access_token) {
            if claims.ipaddr == client_ip {
                return Ok(AuthenticatedUser {
                    claims,
                    user_id: None,
                    refresh_jti: None,
                });
            }
        }
    }

    // Slow path: refresh the access token
    let refresh_token =
        get_cookie(&parts.headers, REFRESH_COOKIE_NAME).ok_or(AuthErrorKind::NotAuthenticated)?;

    let refresh_claims = state
        .jwt()
        .validate_refresh_token(refresh_token)
        .map_err(|_| AuthErrorKind::InvalidToken)?;

    let active_token = state
        .db()
        .tokens()
        .get_by_jti(&refresh_claims.jti)
        .await
        .map_err(|e| {
            tracing::error!("Failed to check token: {}", e);
            AuthErrorKind::DatabaseError
        })?
        .ok_or(AuthErrorKind::TokenRevoked)?;

    let user = state
        .db()
        .users()
        .get_by_uuid(&refresh_claims.sub)
        .await
        .map_err(|e| {
            tracing::error!("Failed to get user: {}", e);
            AuthErrorKind::DatabaseError
        })?
        .ok_or(AuthErrorKind::UserNotFound)?;

    if !user.activated {
        return Err(AuthErrorKind::AccountNotActivated);
    }

    if active_token.last_ip.as_ref() != Some(&client_ip) {
        if let Err(e) = state
            .db()
            .tokens()
            .update_ip(&refresh_claims.jti, &client_ip)
            .await
        {
            tracing::warn!("Failed to update token IP: {}", e);
        }
    }

    // Fire ip-change hook to notify plugins.
    let ip_change_hook = Hook::Server(ServerHook::IpChange);
    if let Some(pm) = server_config::plugin_manager().filter(|pm| pm.has_hook(&ip_change_hook)) {
        let old_ip = active_token.last_ip.clone().unwrap_or_default();
        let new_ip = client_ip.clone();
        let user_uuid = refresh_claims.sub.clone();
        let pm = pm.clone();
        tokio::spawn(async move {
            pm.fire_hook(
                ip_change_hook,
                vec![
                    ("old_ip".into(), old_ip),
                    ("new_ip".into(), new_ip),
                    ("user_uuid".into(), user_uuid),
                ],
            )
            .await;
        });
    }

    let access_result = state
        .jwt()
        .generate_access_token(&user.uuid, &user.username, user.role, &client_ip)
        .map_err(|e| {
            tracing::error!("Failed to generate access token: {}", e);
            AuthErrorKind::DatabaseError
        })?;

    let secure = if server_config::secure_cookies() {
        "; Secure"
    } else {
        ""
    };
    let new_cookie = format!(
        "{}={}; HttpOnly; SameSite=Strict; Path=/; Max-Age={}{}",
        ACCESS_COOKIE_NAME, access_result.token, access_result.duration, secure
    );
    let _ = NEW_ACCESS_TOKEN_COOKIE.try_with(|cell| {
        cell.borrow_mut().replace(new_cookie);
    });

    let claims = state
        .jwt()
        .validate_access_token(&access_result.token)
        .map_err(|_| AuthErrorKind::DatabaseError)?;

    Ok(AuthenticatedUser {
        claims,
        user_id: Some(user.id),
        refresh_jti: Some(refresh_claims.jti),
    })
}

/// Resolve the database user ID, looking it up if not already known from a refresh.
async fn ensure_activated<S>(
    user: AuthenticatedUser,
    state: &S,
) -> Result<(ActivatedAuthenticatedUser, Option<String>), ApiAuthError>
where
    S: HasAuthBackend + Send + Sync,
{
    let id = match user.user_id {
        Some(id) => id,
        None => {
            let db_user = state
                .db()
                .users()
                .get_by_uuid(&user.claims.sub)
                .await
                .map_err(|_| ApiAuthError::new(AuthErrorKind::DatabaseError))?
                .ok_or(ApiAuthError::new(AuthErrorKind::UserNotFound))?;

            if !db_user.activated {
                return Err(ApiAuthError::new(AuthErrorKind::AccountNotActivated));
            }
            db_user.id
        }
    };

    Ok((
        ActivatedAuthenticatedUser {
            claims: user.claims,
            user_id: id,
        },
        user.refresh_jti,
    ))
}

/// API authentication extractor with role-based access control.
/// Returns JSON errors on failure.
pub struct Auth<R: RoleConstraint>(pub ActivatedAuthenticatedUser, pub PhantomData<R>);

impl<R: RoleConstraint> Deref for Auth<R> {
    type Target = ActivatedAuthenticatedUser;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<S, R> FromRequestParts<S> for Auth<R>
where
    S: HasAuthBackend + Send + Sync,
    R: RoleConstraint,
{
    type Rejection = ApiAuthError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let user = authenticate_request(parts, state)
            .await
            .map_err(|kind| ApiAuthError::new(kind))?;

        if !R::check(user.claims.role) {
            return Err(ApiAuthError::new(AuthErrorKind::InsufficientRole));
        }

        let (activated, _) = ensure_activated(user, state).await?;
        Ok(Auth(activated, PhantomData))
    }
}

/// Like `Auth<R>` but also provides the current session's refresh token JTI.
///
/// Requires a valid refresh token. If the access token was valid on its own
/// (no refresh needed), the refresh token is validated to obtain the JTI.
pub struct AuthWithSession<R: RoleConstraint>(pub AuthenticatedUserWithSession, pub PhantomData<R>);

impl<R: RoleConstraint> Deref for AuthWithSession<R> {
    type Target = AuthenticatedUserWithSession;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<S, R> FromRequestParts<S> for AuthWithSession<R>
where
    S: HasAuthBackend + Send + Sync,
    R: RoleConstraint,
{
    type Rejection = ApiAuthError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let user = authenticate_request(parts, state)
            .await
            .map_err(|kind| ApiAuthError::new(kind))?;

        if !R::check(user.claims.role) {
            return Err(ApiAuthError::new(AuthErrorKind::InsufficientRole));
        }

        // If authenticate_request already went through the refresh path, we have the JTI.
        // Otherwise, validate the refresh token now to get it.
        let refresh_jti = match user.refresh_jti.clone() {
            Some(jti) => jti,
            None => {
                let refresh_token = get_cookie(&parts.headers, REFRESH_COOKIE_NAME)
                    .ok_or(ApiAuthError::new(AuthErrorKind::NotAuthenticated))?;

                let refresh_claims = state
                    .jwt()
                    .validate_refresh_token(refresh_token)
                    .map_err(|_| ApiAuthError::new(AuthErrorKind::InvalidToken))?;

                state
                    .db()
                    .tokens()
                    .get_by_jti(&refresh_claims.jti)
                    .await
                    .map_err(|_| ApiAuthError::new(AuthErrorKind::DatabaseError))?
                    .ok_or(ApiAuthError::new(AuthErrorKind::TokenRevoked))?;

                refresh_claims.jti
            }
        };

        let (activated, _) = ensure_activated(user, state).await?;

        Ok(AuthWithSession(
            AuthenticatedUserWithSession {
                claims: activated.claims,
                user_id: activated.user_id,
                refresh_jti,
            },
            PhantomData,
        ))
    }
}

/// Optional authentication extractor — never fails, returns `Option<AuthenticatedUser>`.
pub struct OptionalAuth(pub Option<AuthenticatedUser>);

impl<S> FromRequestParts<S> for OptionalAuth
where
    S: HasAuthBackend + Send + Sync,
{
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        Ok(OptionalAuth(authenticate_request(parts, state).await.ok()))
    }
}

/// Asset authentication extractor that redirects to login on failure.
/// Unlike `Auth<R>`, does not return JSON errors.
pub struct ProtectedAsset<R: RoleConstraint>(pub AuthenticatedUser, pub PhantomData<R>);

impl<S, R> FromRequestParts<S> for ProtectedAsset<R>
where
    S: HasAssetAuthBackend + Send + Sync,
    R: RoleConstraint,
{
    type Rejection = AssetAuthError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let login_path = state.login_path().to_string();
        let user = authenticate_request(parts, state)
            .await
            .map_err(|_| AssetAuthError {
                login_path: login_path.clone(),
            })?;

        if !R::check(user.claims.role) {
            return Err(AssetAuthError { login_path });
        }

        Ok(ProtectedAsset(user, PhantomData))
    }
}
