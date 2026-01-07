//! Tests for the Encryption API.

mod common;

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use crowchiper::{ServerConfig, create_app, db::Database, jwt::JwtConfig};
use tower::ServiceExt;
use url::Url;

/// Create a test app and return (app, db, jwt_config).
async fn create_test_app() -> (axum::Router, Database, JwtConfig) {
    let db = Database::open(":memory:")
        .await
        .expect("Failed to open test database");
    let jwt_secret = b"test-jwt-secret".to_vec();
    let jwt_config = JwtConfig::new(&jwt_secret);
    let config = ServerConfig {
        base: None,
        db: db.clone(),
        rp_id: "localhost".to_string(),
        rp_origin: Url::parse("http://localhost").expect("Invalid URL"),
        jwt_secret,
        secure_cookies: false,
        no_signup: false,
    };
    (create_app(&config), db, jwt_config)
}

/// Create a user and return (user_id, jwt_token).
async fn create_authenticated_user(
    db: &Database,
    jwt: &JwtConfig,
    username: &str,
) -> (i64, String) {
    let uuid = uuid::Uuid::new_v4().to_string();
    let id = db.users().create(&uuid, username).await.unwrap();
    db.users().activate(id).await.unwrap();
    let token = jwt
        .generate_token(&uuid, username, crowchiper::db::UserRole::User)
        .unwrap();
    (id, token)
}

fn auth_cookie(token: &str) -> String {
    format!("auth_token={}", token)
}

#[tokio::test]
async fn test_get_encryption_settings_new_user() {
    let (app, db, jwt) = create_test_app().await;
    let (_, token) = create_authenticated_user(&db, &jwt, "alice").await;

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/encryption/settings")
                .header("cookie", auth_cookie(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    // New user should have setup_done=false and no prf_salt
    assert_eq!(json["setup_done"], false);
    assert_eq!(json["prf_salt"], serde_json::Value::Null);
}

#[tokio::test]
async fn test_get_encryption_settings_unauthenticated() {
    let (app, _, _) = create_test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/encryption/settings")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // Should redirect to login (302) or return unauthorized
    assert!(
        response.status() == StatusCode::FOUND
            || response.status() == StatusCode::UNAUTHORIZED
            || response.status() == StatusCode::SEE_OTHER
    );
}

#[tokio::test]
async fn test_setup_encryption() {
    let (app, db, jwt) = create_test_app().await;
    let (_, token) = create_authenticated_user(&db, &jwt, "alice").await;

    // Set up encryption (server generates salt)
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/encryption/setup")
                .header("cookie", auth_cookie(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    // Should return the generated PRF salt
    assert!(json["prf_salt"].is_string());
    let prf_salt = json["prf_salt"].as_str().unwrap();
    assert!(!prf_salt.is_empty());

    // Verify settings were saved
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/encryption/settings")
                .header("cookie", auth_cookie(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["setup_done"], true);
    assert_eq!(json["prf_salt"], prf_salt);
}

#[tokio::test]
async fn test_setup_encryption_already_set_up() {
    let (app, db, jwt) = create_test_app().await;
    let (_, token) = create_authenticated_user(&db, &jwt, "alice").await;

    // First setup should succeed
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/encryption/setup")
                .header("cookie", auth_cookie(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);

    // Second setup should fail with conflict
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/encryption/setup")
                .header("cookie", auth_cookie(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CONFLICT);
}

#[tokio::test]
async fn test_skip_encryption() {
    let (app, db, jwt) = create_test_app().await;
    let (_, token) = create_authenticated_user(&db, &jwt, "alice").await;

    // Skip encryption
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/encryption/skip")
                .header("cookie", auth_cookie(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    // Verify settings - setup_done=true but no prf_salt
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/encryption/settings")
                .header("cookie", auth_cookie(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["setup_done"], true);
    assert_eq!(json["prf_salt"], serde_json::Value::Null);
}

#[tokio::test]
async fn test_skip_encryption_already_set_up() {
    let (app, db, jwt) = create_test_app().await;
    let (_, token) = create_authenticated_user(&db, &jwt, "alice").await;

    // First skip
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/encryption/skip")
                .header("cookie", auth_cookie(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    // Second skip should fail with conflict
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/encryption/skip")
                .header("cookie", auth_cookie(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CONFLICT);
}

#[tokio::test]
async fn test_setup_encryption_unauthenticated() {
    let (app, _, _) = create_test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/encryption/setup")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // Should redirect to login or return unauthorized
    assert!(
        response.status() == StatusCode::FOUND
            || response.status() == StatusCode::UNAUTHORIZED
            || response.status() == StatusCode::SEE_OTHER
    );
}

#[tokio::test]
async fn test_encryption_settings_isolation() {
    let (app, db, jwt) = create_test_app().await;
    let (_, alice_token) = create_authenticated_user(&db, &jwt, "alice").await;
    let (_, bob_token) = create_authenticated_user(&db, &jwt, "bob").await;

    // Alice sets up encryption
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/encryption/setup")
                .header("cookie", auth_cookie(&alice_token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let alice_json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let alice_salt = alice_json["prf_salt"].as_str().unwrap();

    // Bob should have no encryption settings
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/encryption/settings")
                .header("cookie", auth_cookie(&bob_token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["setup_done"], false);
    assert_eq!(json["prf_salt"], serde_json::Value::Null);

    // Alice should still have her settings
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/encryption/settings")
                .header("cookie", auth_cookie(&alice_token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["setup_done"], true);
    assert_eq!(json["prf_salt"], alice_salt);
}
