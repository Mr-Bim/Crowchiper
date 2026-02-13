//! Tests for the Posts API.

mod common;

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
#[cfg(feature = "test-mode")]
use crowchiper::local_ip_extractor;
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
        csp_nonce: false,
        ip_extractor: Some(local_ip_extractor()),
        plugin_manager: None,
    };
    (create_app(&config), db, jwt_config)
}

/// Create a user and return (user_id, access_token, refresh_token).
async fn create_authenticated_user(
    db: &Database,
    jwt: &JwtConfig,
    username: &str,
) -> (i64, String, String) {
    let uuid = uuid::Uuid::new_v4().to_string();
    let id = db.users().create(&uuid, username).await.unwrap();
    db.users().activate(id).await.unwrap();

    // Generate access token (stateless, no DB storage)
    let access_result = jwt
        .generate_access_token(&uuid, username, crowchiper::db::UserRole::User, "127.0.0.1")
        .unwrap();

    // Generate refresh token and store in DB
    let refresh_result = jwt
        .generate_refresh_token(&uuid, username, crowchiper::db::UserRole::User)
        .unwrap();
    db.tokens()
        .create(
            &refresh_result.jti,
            id,
            None,
            refresh_result.issued_at,
            refresh_result.expires_at,
        )
        .await
        .unwrap();

    (id, access_result.token, refresh_result.token)
}

fn auth_cookies(access_token: &str, refresh_token: &str) -> String {
    format!(
        "access_token={}; refresh_token={}",
        access_token, refresh_token
    )
}

/// Test IP address used for authentication tokens
const TEST_IP: &str = "127.0.0.1";

#[tokio::test]
async fn test_list_posts_empty() {
    let (app, db, jwt) = create_test_app().await;
    let (_, access, refresh) = create_authenticated_user(&db, &jwt, "alice").await;

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/posts")
                .header("cookie", auth_cookies(&access, &refresh))
                .header("x-forwarded-for", TEST_IP)
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

    assert!(json.as_array().unwrap().is_empty());
}

#[tokio::test]
async fn test_create_post() {
    let (app, db, jwt) = create_test_app().await;
    let (_, access, refresh) = create_authenticated_user(&db, &jwt, "alice").await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/posts")
                .header("content-type", "application/json")
                .header("cookie", auth_cookies(&access, &refresh))
                .header("x-forwarded-for", TEST_IP)
                .body(Body::from(
                    r#"{"title": "My First Post", "content": "Hello, world!"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert!(json["uuid"].as_str().is_some());
}

#[tokio::test]
async fn test_create_post_without_title() {
    let (app, db, jwt) = create_test_app().await;
    let (_, access, refresh) = create_authenticated_user(&db, &jwt, "alice").await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/posts")
                .header("content-type", "application/json")
                .header("cookie", auth_cookies(&access, &refresh))
                .header("x-forwarded-for", TEST_IP)
                .body(Body::from(r#"{"content": "Just content"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);
}

#[tokio::test]
async fn test_get_post() {
    let (app, db, jwt) = create_test_app().await;
    let (user_id, access, refresh) = create_authenticated_user(&db, &jwt, "alice").await;

    // Create a post directly in the database
    let post_uuid = db
        .posts()
        .create(
            user_id,
            Some("Test Post"),
            false,
            None,
            "Test content",
            false,
            None,
            None,
            None,
        )
        .await
        .unwrap();

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/posts/{}", post_uuid))
                .header("cookie", auth_cookies(&access, &refresh))
                .header("x-forwarded-for", TEST_IP)
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

    assert_eq!(json["uuid"], post_uuid);
    assert_eq!(json["title"], "Test Post");
    assert_eq!(json["content"], "Test content");
}

#[tokio::test]
async fn test_get_post_not_found() {
    let (app, db, jwt) = create_test_app().await;
    let (_, access, refresh) = create_authenticated_user(&db, &jwt, "alice").await;

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/posts/00000000-0000-0000-0000-000000000000")
                .header("cookie", auth_cookies(&access, &refresh))
                .header("x-forwarded-for", TEST_IP)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_update_post() {
    let (app, db, jwt) = create_test_app().await;
    let (user_id, access, refresh) = create_authenticated_user(&db, &jwt, "alice").await;

    let post_uuid = db
        .posts()
        .create(
            user_id,
            Some("Original"),
            false,
            None,
            "Original content",
            false,
            None,
            None,
            None,
        )
        .await
        .unwrap();

    let response = app
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/posts/{}", post_uuid))
                .header("content-type", "application/json")
                .header("cookie", auth_cookies(&access, &refresh))
                .header("x-forwarded-for", TEST_IP)
                .body(Body::from(
                    r#"{"title": "Updated", "content": "Updated content"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    // Verify the update
    let post = db
        .posts()
        .get_by_uuid(&post_uuid, user_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(post.title, Some("Updated".to_string()));
    assert_eq!(post.content, "Updated content");
}

#[tokio::test]
async fn test_delete_post() {
    let (app, db, jwt) = create_test_app().await;
    let (user_id, access, refresh) = create_authenticated_user(&db, &jwt, "alice").await;

    let post_uuid = db
        .posts()
        .create(
            user_id,
            Some("To Delete"),
            false,
            None,
            "Content",
            false,
            None,
            None,
            None,
        )
        .await
        .unwrap();

    let response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/posts/{}", post_uuid))
                .header("cookie", auth_cookies(&access, &refresh))
                .header("x-forwarded-for", TEST_IP)
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
    assert_eq!(json["deleted"], true);
    assert_eq!(json["children_deleted"], 0);

    // Verify deletion
    let post = db.posts().get_by_uuid(&post_uuid, user_id).await.unwrap();
    assert!(post.is_none());
}

#[tokio::test]
async fn test_list_posts_returns_user_posts_only() {
    let (app, db, jwt) = create_test_app().await;
    let (alice_id, alice_access, alice_refresh) =
        create_authenticated_user(&db, &jwt, "alice").await;
    let (bob_id, _bob_access, _bob_refresh) = create_authenticated_user(&db, &jwt, "bob").await;

    // Create posts for both users
    db.posts()
        .create(
            alice_id,
            Some("Alice Post 1"),
            false,
            None,
            "Content",
            false,
            None,
            None,
            None,
        )
        .await
        .unwrap();
    db.posts()
        .create(
            alice_id,
            Some("Alice Post 2"),
            false,
            None,
            "Content",
            false,
            None,
            None,
            None,
        )
        .await
        .unwrap();
    db.posts()
        .create(
            bob_id,
            Some("Bob Post"),
            false,
            None,
            "Content",
            false,
            None,
            None,
            None,
        )
        .await
        .unwrap();

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/posts")
                .header("cookie", auth_cookies(&alice_access, &alice_refresh))
                .header("x-forwarded-for", TEST_IP)
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

    // Alice should only see her 2 posts
    assert_eq!(json.as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn test_cannot_access_other_users_post() {
    let (app, db, jwt) = create_test_app().await;
    let (alice_id, _alice_access, _alice_refresh) =
        create_authenticated_user(&db, &jwt, "alice").await;
    let (_, bob_access, bob_refresh) = create_authenticated_user(&db, &jwt, "bob").await;

    // Alice creates a post
    let post_uuid = db
        .posts()
        .create(
            alice_id,
            Some("Alice Secret"),
            false,
            None,
            "Secret content",
            false,
            None,
            None,
            None,
        )
        .await
        .unwrap();

    // Bob tries to get Alice's post
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/posts/{}", post_uuid))
                .header("cookie", auth_cookies(&bob_access, &bob_refresh))
                .header("x-forwarded-for", TEST_IP)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_cannot_update_other_users_post() {
    let (app, db, jwt) = create_test_app().await;
    let (alice_id, _alice_access, _alice_refresh) =
        create_authenticated_user(&db, &jwt, "alice").await;
    let (_, bob_access, bob_refresh) = create_authenticated_user(&db, &jwt, "bob").await;

    let post_uuid = db
        .posts()
        .create(
            alice_id,
            Some("Alice Post"),
            false,
            None,
            "Content",
            false,
            None,
            None,
            None,
        )
        .await
        .unwrap();

    // Bob tries to update Alice's post
    let response = app
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/posts/{}", post_uuid))
                .header("content-type", "application/json")
                .header("cookie", auth_cookies(&bob_access, &bob_refresh))
                .header("x-forwarded-for", TEST_IP)
                .body(Body::from(r#"{"title": "Hacked", "content": "Hacked"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    // Verify post is unchanged
    let post = db
        .posts()
        .get_by_uuid(&post_uuid, alice_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(post.title, Some("Alice Post".to_string()));
}

#[tokio::test]
async fn test_cannot_delete_other_users_post() {
    let (app, db, jwt) = create_test_app().await;
    let (alice_id, _alice_access, _alice_refresh) =
        create_authenticated_user(&db, &jwt, "alice").await;
    let (_, bob_access, bob_refresh) = create_authenticated_user(&db, &jwt, "bob").await;

    let post_uuid = db
        .posts()
        .create(
            alice_id,
            Some("Alice Post"),
            false,
            None,
            "Content",
            false,
            None,
            None,
            None,
        )
        .await
        .unwrap();

    // Bob tries to delete Alice's post
    let response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/posts/{}", post_uuid))
                .header("cookie", auth_cookies(&bob_access, &bob_refresh))
                .header("x-forwarded-for", TEST_IP)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    // Verify post still exists
    let post = db.posts().get_by_uuid(&post_uuid, alice_id).await.unwrap();
    assert!(post.is_some());
}

#[tokio::test]
async fn test_unauthenticated_access_denied() {
    let (app, _, _) = create_test_app().await;

    // Try to list posts without auth
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/posts")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_invalid_token_denied() {
    let (app, _, _) = create_test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/posts")
                .header("cookie", "auth_token=invalid-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_posts_with_base_path() {
    let db = Database::open(":memory:")
        .await
        .expect("Failed to open test database");
    let jwt_secret = b"test-jwt-secret".to_vec();
    let jwt = JwtConfig::new(&jwt_secret);
    let config = ServerConfig {
        base: Some("/app".to_string()),
        db: db.clone(),
        rp_id: "localhost".to_string(),
        rp_origin: Url::parse("http://localhost").expect("Invalid URL"),
        jwt_secret,
        secure_cookies: false,
        no_signup: false,
        csp_nonce: false,
        ip_extractor: Some(local_ip_extractor()),
        plugin_manager: None,
    };
    let app = create_app(&config);

    let (_, access, refresh) = create_authenticated_user(&db, &jwt, "alice").await;

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/app/api/posts")
                .header("cookie", auth_cookies(&access, &refresh))
                .header("x-forwarded-for", TEST_IP)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_create_encrypted_post() {
    let (app, db, jwt) = create_test_app().await;
    let (user_id, access, refresh) = create_authenticated_user(&db, &jwt, "alice").await;

    // Enable encryption for user (required to submit encrypted posts)
    let prf_salt = vec![0u8; 32];
    db.encryption_settings()
        .create(user_id, &prf_salt)
        .await
        .unwrap();

    // Create a post with encrypted flags set
    let encrypted_content = r#"{"v":1,"ct":"abc123","iv":"def456"}"#;
    let encrypted_title = r#"{"v":1,"ct":"title123","iv":"title456"}"#;

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/posts")
                .header("content-type", "application/json")
                .header("cookie", auth_cookies(&access, &refresh))
                .header("x-forwarded-for", TEST_IP)
                .body(Body::from(format!(
                    r#"{{"title": "{}", "title_encrypted": true, "content": "{}", "content_encrypted": true}}"#,
                    encrypted_title.replace('"', "\\\""),
                    encrypted_content.replace('"', "\\\"")
                )))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    let post_uuid = json["uuid"].as_str().unwrap();

    // Get the post and verify encrypted flags
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/posts/{}", post_uuid))
                .header("cookie", auth_cookies(&access, &refresh))
                .header("x-forwarded-for", TEST_IP)
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

    assert_eq!(json["title_encrypted"], true);
    assert_eq!(json["content_encrypted"], true);
    // The server stores the content as-is (opaque)
    assert!(json["content"].as_str().unwrap().contains("ct"));
}

#[tokio::test]
async fn test_update_post_encryption_flags() {
    let (app, db, jwt) = create_test_app().await;
    let (user_id, access, refresh) = create_authenticated_user(&db, &jwt, "alice").await;

    // Enable encryption for user (required to submit encrypted posts)
    let prf_salt = vec![0u8; 32];
    db.encryption_settings()
        .create(user_id, &prf_salt)
        .await
        .unwrap();

    // Create an encrypted post (since user has encryption enabled)
    let post_uuid = db
        .posts()
        .create(
            user_id,
            Some("Encrypted Title"),
            true,
            Some("title_iv"),
            "encrypted content",
            true,
            Some("content_iv"),
            Some(1),
            None,
        )
        .await
        .unwrap();

    // Update to different encrypted content
    let encrypted_content = r#"{"v":1,"ct":"encrypted","iv":"iv123"}"#;

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/posts/{}", post_uuid))
                .header("content-type", "application/json")
                .header("cookie", auth_cookies(&access, &refresh))
                .header("x-forwarded-for", TEST_IP)
                .body(Body::from(format!(
                    r#"{{"title": "Encrypted Title", "title_encrypted": true, "content": "{}", "content_encrypted": true}}"#,
                    encrypted_content.replace('"', "\\\"")
                )))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    // Verify the flags were updated
    let post = db
        .posts()
        .get_by_uuid(&post_uuid, user_id)
        .await
        .unwrap()
        .unwrap();
    assert!(post.title_encrypted);
    assert!(post.content_encrypted);
}

#[tokio::test]
async fn test_list_posts_includes_encrypted_flags() {
    let (app, db, jwt) = create_test_app().await;
    let (user_id, access, refresh) = create_authenticated_user(&db, &jwt, "alice").await;

    // Create an encrypted post directly in DB
    let encrypted_title = "enc_title_ciphertext";
    db.posts()
        .create(
            user_id,
            Some(encrypted_title),
            true,
            Some("title_iv"),
            "encrypted content",
            true,
            Some("content_iv"),
            Some(1),
            None,
        )
        .await
        .unwrap();

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/posts")
                .header("cookie", auth_cookies(&access, &refresh))
                .header("x-forwarded-for", TEST_IP)
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

    // List should return the encrypted title as-is with encrypted flags
    let posts = json.as_array().unwrap();
    assert_eq!(posts.len(), 1);
    assert_eq!(posts[0]["title"], "enc_title_ciphertext");
    assert_eq!(posts[0]["title_encrypted"], true);
    assert_eq!(posts[0]["title_iv"], "title_iv");
    assert_eq!(posts[0]["content_encrypted"], true);
    assert_eq!(posts[0]["encryption_version"], 1);
}

#[tokio::test]
async fn test_reorder_posts() {
    let (app, db, jwt) = create_test_app().await;
    let (user_id, access, refresh) = create_authenticated_user(&db, &jwt, "alice").await;

    // Create 3 posts - they will be at positions 0, 0, 0 initially
    // (each new post shifts others down and takes position 0)
    let post1 = db
        .posts()
        .create(
            user_id,
            Some("Post 1"),
            false,
            None,
            "C1",
            false,
            None,
            None,
            None,
        )
        .await
        .unwrap();
    let post2 = db
        .posts()
        .create(
            user_id,
            Some("Post 2"),
            false,
            None,
            "C2",
            false,
            None,
            None,
            None,
        )
        .await
        .unwrap();
    let post3 = db
        .posts()
        .create(
            user_id,
            Some("Post 3"),
            false,
            None,
            "C3",
            false,
            None,
            None,
            None,
        )
        .await
        .unwrap();

    // Current order should be: post3 (0), post2 (1), post1 (2)
    // Reorder to: post1, post3, post2
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/posts/reorder")
                .header("content-type", "application/json")
                .header("cookie", auth_cookies(&access, &refresh))
                .header("x-forwarded-for", TEST_IP)
                .body(Body::from(format!(
                    r#"{{"uuids": ["{}", "{}", "{}"]}}"#,
                    post1, post3, post2
                )))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    // Verify the new order by listing posts
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/posts")
                .header("cookie", auth_cookies(&access, &refresh))
                .header("x-forwarded-for", TEST_IP)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let posts = json.as_array().unwrap();

    assert_eq!(posts.len(), 3);
    assert_eq!(posts[0]["uuid"], post1);
    assert_eq!(posts[0]["position"], 0);
    assert_eq!(posts[1]["uuid"], post3);
    assert_eq!(posts[1]["position"], 1);
    assert_eq!(posts[2]["uuid"], post2);
    assert_eq!(posts[2]["position"], 2);
}

#[tokio::test]
async fn test_reorder_posts_empty_list() {
    let (app, db, jwt) = create_test_app().await;
    let (_, access, refresh) = create_authenticated_user(&db, &jwt, "alice").await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/posts/reorder")
                .header("content-type", "application/json")
                .header("cookie", auth_cookies(&access, &refresh))
                .header("x-forwarded-for", TEST_IP)
                .body(Body::from(r#"{"uuids": []}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn test_reorder_cannot_affect_other_users_posts() {
    let (app, db, jwt) = create_test_app().await;
    let (alice_id, _alice_access, _alice_refresh) =
        create_authenticated_user(&db, &jwt, "alice").await;
    let (_, bob_access, bob_refresh) = create_authenticated_user(&db, &jwt, "bob").await;

    // Alice creates posts
    let post1 = db
        .posts()
        .create(
            alice_id,
            Some("Alice 1"),
            false,
            None,
            "C1",
            false,
            None,
            None,
            None,
        )
        .await
        .unwrap();
    let post2 = db
        .posts()
        .create(
            alice_id,
            Some("Alice 2"),
            false,
            None,
            "C2",
            false,
            None,
            None,
            None,
        )
        .await
        .unwrap();

    // Bob tries to reorder Alice's posts
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/posts/reorder")
                .header("content-type", "application/json")
                .header("cookie", auth_cookies(&bob_access, &bob_refresh))
                .header("x-forwarded-for", TEST_IP)
                .body(Body::from(format!(
                    r#"{{"uuids": ["{}", "{}"]}}"#,
                    post2, post1
                )))
                .unwrap(),
        )
        .await
        .unwrap();

    // Request succeeds but no posts are updated (Bob doesn't own them)
    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    // Verify Alice's posts are unchanged (still in original order)
    let posts = db.posts().list_by_user(alice_id).await.unwrap();
    // post2 was created last so it's at position 0
    assert_eq!(posts[0].uuid, post2);
    assert_eq!(posts[1].uuid, post1);
}

#[tokio::test]
async fn test_new_post_inserted_at_top() {
    let (app, db, jwt) = create_test_app().await;
    let (user_id, access, refresh) = create_authenticated_user(&db, &jwt, "alice").await;

    // Create first post
    let post1 = db
        .posts()
        .create(
            user_id,
            Some("Post 1"),
            false,
            None,
            "C1",
            false,
            None,
            None,
            None,
        )
        .await
        .unwrap();

    // Create second post via API
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/posts")
                .header("content-type", "application/json")
                .header("cookie", auth_cookies(&access, &refresh))
                .header("x-forwarded-for", TEST_IP)
                .body(Body::from(r#"{"title": "Post 2", "content": "C2"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let post2 = json["uuid"].as_str().unwrap();

    // List posts - new post should be first
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/posts")
                .header("cookie", auth_cookies(&access, &refresh))
                .header("x-forwarded-for", TEST_IP)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let posts = json.as_array().unwrap();

    assert_eq!(posts.len(), 2);
    assert_eq!(posts[0]["uuid"], post2);
    assert_eq!(posts[0]["position"], 0);
    assert_eq!(posts[1]["uuid"], post1);
    assert_eq!(posts[1]["position"], 1);
}

#[tokio::test]
async fn test_list_posts_includes_position() {
    let (app, db, jwt) = create_test_app().await;
    let (user_id, access, refresh) = create_authenticated_user(&db, &jwt, "alice").await;

    db.posts()
        .create(
            user_id,
            Some("Post"),
            false,
            None,
            "Content",
            false,
            None,
            None,
            None,
        )
        .await
        .unwrap();

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/posts")
                .header("cookie", auth_cookies(&access, &refresh))
                .header("x-forwarded-for", TEST_IP)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let posts = json.as_array().unwrap();

    assert_eq!(posts.len(), 1);
    assert_eq!(posts[0]["position"], 0);
}
