// WebSocket disabled - tests kept for future re-enablement
#![cfg(feature = "disabled")]

mod common;

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use crowchiper::{ServerConfig, create_app, db::Database};
use tower::ServiceExt;
use url::Url;

async fn create_test_app() -> (axum::Router, Database, crowchiper::jwt::JwtConfig) {
    let db = Database::open(":memory:")
        .await
        .expect("Failed to open test database");
    let jwt_secret = b"test-jwt-secret".to_vec();
    let jwt = crowchiper::jwt::JwtConfig::new(&jwt_secret);
    let config = ServerConfig {
        base: None,
        db: db.clone(),
        rp_id: "localhost".to_string(),
        rp_origin: Url::parse("http://localhost").expect("Invalid URL"),
        jwt_secret,
        secure_cookies: false,
        no_signup: false,
    };
    (create_app(&config), db, jwt)
}

#[tokio::test]
async fn test_ws_requires_auth() {
    let (app, _, _) = create_test_app().await;

    // Try to connect without auth cookie
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/ws")
                .header("connection", "upgrade")
                .header("upgrade", "websocket")
                .header("sec-websocket-version", "13")
                .header("sec-websocket-key", "dGhlIHNhbXBsZSBub25jZQ==")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_ws_rejects_invalid_token() {
    let (app, _, _) = create_test_app().await;

    // Try to connect with invalid token
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/ws")
                .header("connection", "upgrade")
                .header("upgrade", "websocket")
                .header("sec-websocket-version", "13")
                .header("sec-websocket-key", "dGhlIHNhbXBsZSBub25jZQ==")
                .header("cookie", "auth_token=invalid-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_ws_rejects_inactive_user() {
    let (app, db, jwt) = create_test_app().await;

    // Create user but don't activate
    let uuid = "00000000-0000-0000-0000-000000000001";
    db.users().create(uuid, "alice").await.unwrap();

    // Generate token for inactive user
    let token = jwt
        .generate_token(uuid, "alice", crowchiper::db::UserRole::User)
        .unwrap();

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/ws")
                .header("connection", "upgrade")
                .header("upgrade", "websocket")
                .header("sec-websocket-version", "13")
                .header("sec-websocket-key", "dGhlIHNhbXBsZSBub25jZQ==")
                .header("cookie", format!("auth_token={}", token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_ws_accepts_valid_auth() {
    let (app, db, jwt) = create_test_app().await;

    // Create and activate user
    let uuid = "00000000-0000-0000-0000-000000000001";
    let id = db.users().create(uuid, "alice").await.unwrap();
    db.users().activate(id).await.unwrap();

    // Generate valid token
    let token = jwt
        .generate_token(uuid, "alice", crowchiper::db::UserRole::User)
        .unwrap();

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/ws")
                .header("connection", "upgrade")
                .header("upgrade", "websocket")
                .header("sec-websocket-version", "13")
                .header("sec-websocket-key", "dGhlIHNhbXBsZSBub25jZQ==")
                .header("cookie", format!("auth_token={}", token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // With valid auth, we get past auth checks. The WebSocket upgrade fails
    // in oneshot tests because they don't support real HTTP upgrades,
    // but we verify auth passed by not getting 401/403.
    // 400 means auth passed but upgrade failed (expected in tests).
    assert!(
        response.status() == StatusCode::SWITCHING_PROTOCOLS
            || response.status() == StatusCode::BAD_REQUEST,
        "Expected 101 or 400, got {}",
        response.status()
    );
    // Most importantly: NOT 401 or 403
    assert_ne!(response.status(), StatusCode::UNAUTHORIZED);
    assert_ne!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_ws_with_base_path() {
    let db = Database::open(":memory:")
        .await
        .expect("Failed to open test database");
    let jwt_secret = b"test-jwt-secret".to_vec();
    let jwt = crowchiper::jwt::JwtConfig::new(&jwt_secret);
    let config = ServerConfig {
        base: Some("/app".to_string()),
        db: db.clone(),
        rp_id: "localhost".to_string(),
        rp_origin: Url::parse("http://localhost").expect("Invalid URL"),
        jwt_secret,
        secure_cookies: false,
        no_signup: false,
    };
    let app = create_app(&config);

    // Create and activate user
    let uuid = "00000000-0000-0000-0000-000000000001";
    let id = db.users().create(uuid, "alice").await.unwrap();
    db.users().activate(id).await.unwrap();

    // Generate valid token
    let token = jwt
        .generate_token(uuid, "alice", crowchiper::db::UserRole::User)
        .unwrap();

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/app/api/ws")
                .header("connection", "upgrade")
                .header("upgrade", "websocket")
                .header("sec-websocket-version", "13")
                .header("sec-websocket-key", "dGhlIHNhbXBsZSBub25jZQ==")
                .header("cookie", format!("auth_token={}", token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // With valid auth, we get past auth checks. 400 means auth passed but
    // upgrade failed (expected in oneshot tests).
    assert!(
        response.status() == StatusCode::SWITCHING_PROTOCOLS
            || response.status() == StatusCode::BAD_REQUEST,
        "Expected 101 or 400, got {}",
        response.status()
    );
    assert_ne!(response.status(), StatusCode::UNAUTHORIZED);
    assert_ne!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_ws_rejects_nonexistent_user() {
    let (app, _, jwt) = create_test_app().await;

    // Generate token for user that doesn't exist
    let uuid = "00000000-0000-0000-0000-000000000999";
    let token = jwt
        .generate_token(uuid, "ghost", crowchiper::db::UserRole::User)
        .unwrap();

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/ws")
                .header("connection", "upgrade")
                .header("upgrade", "websocket")
                .header("sec-websocket-version", "13")
                .header("sec-websocket-key", "dGhlIHNhbXBsZSBub25jZQ==")
                .header("cookie", format!("auth_token={}", token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}
