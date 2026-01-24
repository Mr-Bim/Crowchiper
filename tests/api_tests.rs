mod common;

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use crowchiper::{ServerConfig, create_app, db::Database};
use tower::ServiceExt;
use url::Url;

async fn create_test_app() -> axum::Router {
    let db = Database::open(":memory:")
        .await
        .expect("Failed to open test database");
    let config = ServerConfig {
        base: None,
        db,
        rp_id: "localhost".to_string(),
        rp_origin: Url::parse("http://localhost").expect("Invalid URL"),
        jwt_secret: b"test-jwt-secret".to_vec(),
        secure_cookies: false,
        no_signup: false,
        csp_nonce: false,
        ip_header: None,
    };
    create_app(&config)
}

#[tokio::test]
async fn test_create_user_success() {
    let app = create_test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/users")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"username": "alice"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["username"], "alice");
    assert!(json["uuid"].as_str().is_some());
}

#[tokio::test]
async fn test_create_user_empty_username() {
    let app = create_test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/users")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"username": ""}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_create_user_invalid_characters() {
    let app = create_test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/users")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"username": "alice@bob"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_create_user_too_long() {
    let app = create_test_app().await;

    let long_name = "a".repeat(33);
    let body = format!(r#"{{"username": "{}"}}"#, long_name);

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/users")
                .header("content-type", "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_create_user_duplicate() {
    let db = Database::open(":memory:")
        .await
        .expect("Failed to open test database");
    let config = ServerConfig {
        base: None,
        db,
        rp_id: "localhost".to_string(),
        rp_origin: Url::parse("http://localhost").expect("Invalid URL"),
        jwt_secret: b"test-jwt-secret".to_vec(),
        secure_cookies: false,
        no_signup: false,
        csp_nonce: false,
        ip_header: None,
    };
    let app = create_app(&config);

    // Create first user
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/users")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"username": "alice"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);

    // Try to create duplicate
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/users")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"username": "alice"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CONFLICT);
}

#[tokio::test]
async fn test_delete_user_success() {
    let db = Database::open(":memory:")
        .await
        .expect("Failed to open test database");
    let config = ServerConfig {
        base: None,
        db,
        rp_id: "localhost".to_string(),
        rp_origin: Url::parse("http://localhost").expect("Invalid URL"),
        jwt_secret: b"test-jwt-secret".to_vec(),
        secure_cookies: false,
        no_signup: false,
        csp_nonce: false,
        ip_header: None,
    };
    let app = create_app(&config);

    // Create user first
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/users")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"username": "alice"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let uuid = json["uuid"].as_str().unwrap();

    // Delete the user
    let response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/users/{}", uuid))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn test_delete_user_not_found() {
    let app = create_test_app().await;

    // Use a valid UUID format that doesn't exist in the database
    let response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/api/users/00000000-0000-0000-0000-000000000000")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_delete_user_invalid_uuid() {
    let app = create_test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/api/users/not-a-valid-uuid")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_delete_activated_user_requires_auth() {
    let db = Database::open(":memory:")
        .await
        .expect("Failed to open test database");
    let config = ServerConfig {
        base: None,
        db: db.clone(),
        rp_id: "localhost".to_string(),
        rp_origin: Url::parse("http://localhost").expect("Invalid URL"),
        jwt_secret: b"test-jwt-secret".to_vec(),
        secure_cookies: false,
        no_signup: false,
        csp_nonce: false,
        ip_header: None,
    };
    let app = create_app(&config);

    // Create and activate user directly with a valid UUID
    let uuid = "00000000-0000-0000-0000-000000000001";
    let id = db.users().create(uuid, "alice").await.unwrap();
    db.users().activate(id).await.unwrap();

    // Try to delete activated user without auth
    let response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/users/{}", uuid))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // Should require authentication
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_delete_activated_user_self() {
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
        csp_nonce: false,
        ip_header: None,
    };
    let app = create_app(&config);

    // Create and activate user
    let uuid = "00000000-0000-0000-0000-000000000001";
    let id = db.users().create(uuid, "alice").await.unwrap();
    db.users().activate(id).await.unwrap();

    // Generate access token for this user
    let access_result = jwt
        .generate_access_token(uuid, "alice", crowchiper::db::UserRole::User, "127.0.0.1")
        .unwrap();

    // Delete own account with auth
    let response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/users/{}", uuid))
                .header("cookie", format!("access_token={}", access_result.token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn test_delete_activated_user_other_forbidden() {
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
        csp_nonce: false,
        ip_header: None,
    };
    let app = create_app(&config);

    // Create and activate alice
    let alice_uuid = "00000000-0000-0000-0000-000000000001";
    let alice_id = db.users().create(alice_uuid, "alice").await.unwrap();
    db.users().activate(alice_id).await.unwrap();

    // Create and activate bob
    let bob_uuid = "00000000-0000-0000-0000-000000000002";
    let bob_id = db.users().create(bob_uuid, "bob").await.unwrap();
    db.users().activate(bob_id).await.unwrap();

    // Generate access token for bob
    let bob_access_result = jwt
        .generate_access_token(bob_uuid, "bob", crowchiper::db::UserRole::User, "127.0.0.1")
        .unwrap();

    // Bob tries to delete alice's account
    let response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/users/{}", alice_uuid))
                .header(
                    "cookie",
                    format!("access_token={}", bob_access_result.token),
                )
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_delete_activated_user_admin() {
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
        csp_nonce: false,
        ip_header: None,
    };
    let app = create_app(&config);

    // Create and activate alice (regular user)
    let alice_uuid = "00000000-0000-0000-0000-000000000001";
    let alice_id = db.users().create(alice_uuid, "alice").await.unwrap();
    db.users().activate(alice_id).await.unwrap();

    // Create and activate admin
    let admin_uuid = "00000000-0000-0000-0000-000000000002";
    let admin_id = db.users().create_admin(admin_uuid, "admin").await.unwrap();
    db.users().activate(admin_id).await.unwrap();

    // Generate access token for admin
    let admin_access_result = jwt
        .generate_access_token(
            admin_uuid,
            "admin",
            crowchiper::db::UserRole::Admin,
            "127.0.0.1",
        )
        .unwrap();

    // Admin deletes alice's account
    let response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/users/{}", alice_uuid))
                .header(
                    "cookie",
                    format!("access_token={}", admin_access_result.token),
                )
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn test_register_start_default_authenticator_type() {
    let db = Database::open(":memory:")
        .await
        .expect("Failed to open test database");
    let config = ServerConfig {
        base: None,
        db: db.clone(),
        rp_id: "localhost".to_string(),
        rp_origin: Url::parse("http://localhost").expect("Invalid URL"),
        jwt_secret: b"test-jwt-secret".to_vec(),
        secure_cookies: false,
        no_signup: false,
        csp_nonce: false,
        ip_header: None,
    };
    let app = create_app(&config);

    // Create user first
    let uuid = "00000000-0000-0000-0000-000000000001";
    db.users().create(uuid, "alice").await.unwrap();

    // Start registration without specifying authenticator_type (defaults to security_key)
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/passkeys/register/start")
                .header("content-type", "application/json")
                .body(Body::from(format!(r#"{{"uuid": "{}"}}"#, uuid)))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    // Should have publicKey with challenge
    assert!(json["publicKey"]["challenge"].as_str().is_some());
    // Default (security_key) should NOT have authenticatorAttachment set to "platform"
    let auth_selection = &json["publicKey"]["authenticatorSelection"];
    assert!(
        auth_selection["authenticatorAttachment"].is_null()
            || auth_selection["authenticatorAttachment"] != "platform",
        "Default should not force platform authenticator"
    );
}

#[tokio::test]
async fn test_register_start_passkey_authenticator_type() {
    let db = Database::open(":memory:")
        .await
        .expect("Failed to open test database");
    let config = ServerConfig {
        base: None,
        db: db.clone(),
        rp_id: "localhost".to_string(),
        rp_origin: Url::parse("http://localhost").expect("Invalid URL"),
        jwt_secret: b"test-jwt-secret".to_vec(),
        secure_cookies: false,
        no_signup: false,
        csp_nonce: false,
        ip_header: None,
    };
    let app = create_app(&config);

    // Create user first
    let uuid = "00000000-0000-0000-0000-000000000001";
    db.users().create(uuid, "alice").await.unwrap();

    // Start registration with passkey type (Google Password Manager flow)
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/passkeys/register/start")
                .header("content-type", "application/json")
                .body(Body::from(format!(
                    r#"{{"uuid": "{}", "authenticator_type": "passkey"}}"#,
                    uuid
                )))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    // Should have publicKey with challenge
    assert!(json["publicKey"]["challenge"].as_str().is_some());
    // Passkey type should have authenticatorAttachment set to "platform"
    let auth_selection = &json["publicKey"]["authenticatorSelection"];
    assert_eq!(
        auth_selection["authenticatorAttachment"], "platform",
        "Passkey type should force platform authenticator"
    );
}

#[tokio::test]
async fn test_register_start_security_key_authenticator_type() {
    let db = Database::open(":memory:")
        .await
        .expect("Failed to open test database");
    let config = ServerConfig {
        base: None,
        db: db.clone(),
        rp_id: "localhost".to_string(),
        rp_origin: Url::parse("http://localhost").expect("Invalid URL"),
        jwt_secret: b"test-jwt-secret".to_vec(),
        secure_cookies: false,
        no_signup: false,
        csp_nonce: false,
        ip_header: None,
    };
    let app = create_app(&config);

    // Create user first
    let uuid = "00000000-0000-0000-0000-000000000001";
    db.users().create(uuid, "alice").await.unwrap();

    // Start registration with security_key type
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/passkeys/register/start")
                .header("content-type", "application/json")
                .body(Body::from(format!(
                    r#"{{"uuid": "{}", "authenticator_type": "security_key"}}"#,
                    uuid
                )))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    // Should have publicKey with challenge
    assert!(json["publicKey"]["challenge"].as_str().is_some());
    // Security key type should NOT have authenticatorAttachment set to "platform"
    let auth_selection = &json["publicKey"]["authenticatorSelection"];
    assert!(
        auth_selection["authenticatorAttachment"].is_null()
            || auth_selection["authenticatorAttachment"] != "platform",
        "Security key should not force platform authenticator"
    );
}

#[tokio::test]
async fn test_create_user_with_base_path() {
    let db = Database::open(":memory:")
        .await
        .expect("Failed to open test database");
    let config = ServerConfig {
        base: Some("/app".to_string()),
        db,
        rp_id: "localhost".to_string(),
        rp_origin: Url::parse("http://localhost").expect("Invalid URL"),
        jwt_secret: b"test-jwt-secret".to_vec(),
        secure_cookies: false,
        no_signup: false,
        csp_nonce: false,
        ip_header: None,
    };
    let app = create_app(&config);

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/app/api/users")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"username": "alice"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);
}

#[tokio::test]
async fn test_no_signup_blocks_user_creation() {
    let db = Database::open(":memory:")
        .await
        .expect("Failed to open test database");
    let config = ServerConfig {
        base: None,
        db,
        rp_id: "localhost".to_string(),
        rp_origin: Url::parse("http://localhost").expect("Invalid URL"),
        jwt_secret: b"test-jwt-secret".to_vec(),
        secure_cookies: false,
        no_signup: true,
        csp_nonce: false,
        ip_header: None,
    };
    let app = create_app(&config);

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/users")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"username": "alice"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    // Should return 404 since the endpoint doesn't exist
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_config_endpoint_returns_no_signup_false() {
    let app = create_test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/config")
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

    assert_eq!(json["no_signup"], false);
}

#[tokio::test]
async fn test_config_endpoint_returns_no_signup_true() {
    let db = Database::open(":memory:")
        .await
        .expect("Failed to open test database");
    let config = ServerConfig {
        base: None,
        db,
        rp_id: "localhost".to_string(),
        rp_origin: Url::parse("http://localhost").expect("Invalid URL"),
        jwt_secret: b"test-jwt-secret".to_vec(),
        secure_cookies: false,
        no_signup: true,
        csp_nonce: false,
        ip_header: None,
    };
    let app = create_app(&config);

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/config")
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

    assert_eq!(json["no_signup"], true);
}

#[tokio::test]
async fn test_no_signup_allows_delete_user() {
    let db = Database::open(":memory:")
        .await
        .expect("Failed to open test database");

    // Create a user directly in the database
    let uuid = "00000000-0000-0000-0000-000000000001";
    db.users().create(uuid, "alice").await.unwrap();

    let config = ServerConfig {
        base: None,
        db,
        rp_id: "localhost".to_string(),
        rp_origin: Url::parse("http://localhost").expect("Invalid URL"),
        jwt_secret: b"test-jwt-secret".to_vec(),
        secure_cookies: false,
        no_signup: true,
        csp_nonce: false,
        ip_header: None,
    };
    let app = create_app(&config);

    // DELETE should still work
    let response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/users/{}", uuid))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NO_CONTENT);
}
