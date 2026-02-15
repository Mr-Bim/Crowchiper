//! Rate limiting for authentication endpoints.
//!
//! Uses a token bucket algorithm with per-IP tracking to prevent brute force attacks.

use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};
use governor::{Quota, RateLimiter, clock::DefaultClock, state::keyed::DefaultKeyedStateStore};
use std::{num::NonZeroU32, sync::Arc};

use crate::auth::extract_client_ip;
use crate::server_config;

/// Per-IP rate limiter for stricter endpoint-specific limiting.
pub type IpLimiter = RateLimiter<String, DefaultKeyedStateStore<String>, DefaultClock>;

/// Rate limiting configuration for authentication endpoints.
#[derive(Clone)]
pub struct RateLimitConfig {
    /// Per-IP limiter for login start endpoints (generous: 10 requests per second)
    pub login_start: Arc<IpLimiter>,
    /// Per-IP limiter for login finish endpoints (strict: 5 requests per 10 seconds)
    pub login_finish: Arc<IpLimiter>,
    /// Per-IP limiter for user creation (strict: 3 requests per minute)
    pub user_create: Arc<IpLimiter>,
}

impl RateLimitConfig {
    /// Create rate limiters with default configuration.
    /// In test mode, limits are much higher to allow rapid test execution.
    pub fn new() -> Self {
        #[cfg(feature = "test-mode")]
        const LOGIN_START_PER_SEC: u32 = 1000;
        #[cfg(not(feature = "test-mode"))]
        const LOGIN_START_PER_SEC: u32 = 10;

        #[cfg(feature = "test-mode")]
        const LOGIN_FINISH_PER_SEC: u32 = 1000;
        #[cfg(not(feature = "test-mode"))]
        const LOGIN_FINISH_PER_SEC: u32 = 1;

        #[cfg(feature = "test-mode")]
        const LOGIN_FINISH_BURST: u32 = 1000;
        #[cfg(not(feature = "test-mode"))]
        const LOGIN_FINISH_BURST: u32 = 5;

        #[cfg(feature = "test-mode")]
        const USER_CREATE_PER_MIN: u32 = 1000;
        #[cfg(not(feature = "test-mode"))]
        const USER_CREATE_PER_MIN: u32 = 3;

        Self {
            // Login start: 10 requests per second per IP (allows normal usage)
            login_start: Arc::new(RateLimiter::keyed(Quota::per_second(
                NonZeroU32::new(LOGIN_START_PER_SEC).unwrap(),
            ))),
            // Login finish: 5 requests per 10 seconds per IP (prevents brute force)
            login_finish: Arc::new(RateLimiter::keyed(
                Quota::per_second(NonZeroU32::new(LOGIN_FINISH_PER_SEC).unwrap())
                    .allow_burst(NonZeroU32::new(LOGIN_FINISH_BURST).unwrap()),
            )),
            // User creation: 3 requests per minute per IP (prevents spam)
            user_create: Arc::new(RateLimiter::keyed(Quota::per_minute(
                NonZeroU32::new(USER_CREATE_PER_MIN).unwrap(),
            ))),
        }
    }
}

/// Middleware for rate limiting login start endpoints.
pub async fn rate_limit_login_start(
    State(config): State<Arc<RateLimitConfig>>,
    request: Request,
    next: Next,
) -> Response {
    let ip = match extract_client_ip(&request, server_config::ip_extractor().as_ref()) {
        Ok(ip) => ip,
        Err(_) => {
            return (StatusCode::FORBIDDEN, "Unable to determine client IP.").into_response();
        }
    };

    match config.login_start.check_key(&ip) {
        Ok(_) => next.run(request).await,
        Err(_) => (
            StatusCode::TOO_MANY_REQUESTS,
            "Too many requests. Please try again later.",
        )
            .into_response(),
    }
}

/// Middleware for rate limiting login finish endpoints.
pub async fn rate_limit_login_finish(
    State(config): State<Arc<RateLimitConfig>>,
    request: Request,
    next: Next,
) -> Response {
    let ip = match extract_client_ip(&request, server_config::ip_extractor().as_ref()) {
        Ok(ip) => ip,
        Err(_) => {
            return (StatusCode::FORBIDDEN, "Unable to determine client IP.").into_response();
        }
    };

    match config.login_finish.check_key(&ip) {
        Ok(_) => next.run(request).await,
        Err(_) => (
            StatusCode::TOO_MANY_REQUESTS,
            "Too many authentication attempts. Please wait before trying again.",
        )
            .into_response(),
    }
}

/// Middleware for rate limiting user creation.
pub async fn rate_limit_user_create(
    State(config): State<Arc<RateLimitConfig>>,
    request: Request,
    next: Next,
) -> Response {
    let ip = match extract_client_ip(&request, server_config::ip_extractor().as_ref()) {
        Ok(ip) => ip,
        Err(_) => {
            return (StatusCode::FORBIDDEN, "Unable to determine client IP.").into_response();
        }
    };

    match config.user_create.check_key(&ip) {
        Ok(_) => next.run(request).await,
        Err(_) => (
            StatusCode::TOO_MANY_REQUESTS,
            "Too many signup attempts. Please wait before trying again.",
        )
            .into_response(),
    }
}
