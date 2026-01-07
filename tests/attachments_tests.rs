//! Tests for the Attachments API.

mod common;

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
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

fn create_test_attachment_body() -> String {
    let image_data = b"fake encrypted image data";
    let thumbnail_data = b"fake encrypted thumbnail";

    let encoded_image = URL_SAFE_NO_PAD.encode(image_data);
    let encoded_thumbnail = URL_SAFE_NO_PAD.encode(thumbnail_data);

    format!(
        r#"{{
            "encrypted_image": "{}",
            "encrypted_image_iv": "image_iv_123",
            "encrypted_thumbnail": "{}",
            "encrypted_thumbnail_iv": "thumb_iv_456",
            "encryption_version": 1
        }}"#,
        encoded_image, encoded_thumbnail
    )
}

#[tokio::test]
async fn test_upload_attachment() {
    let (app, db, jwt) = create_test_app().await;
    let (_, token) = create_authenticated_user(&db, &jwt, "alice").await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/attachments")
                .header("content-type", "application/json")
                .header("cookie", auth_cookie(&token))
                .body(Body::from(create_test_attachment_body()))
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
async fn test_get_attachment() {
    let (app, db, jwt) = create_test_app().await;
    let (user_id, token) = create_authenticated_user(&db, &jwt, "alice").await;

    // Create an attachment directly in the database
    let attachment_uuid = db
        .attachments()
        .create(
            user_id,
            b"encrypted_image_data",
            "image_iv",
            b"encrypted_thumbnail",
            "thumb_iv",
            1,
        )
        .await
        .unwrap();

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/attachments/{}", attachment_uuid))
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

    assert!(json["encrypted_image"].as_str().is_some());
    assert_eq!(json["iv"], "image_iv");
}

#[tokio::test]
async fn test_get_thumbnail() {
    let (app, db, jwt) = create_test_app().await;
    let (user_id, token) = create_authenticated_user(&db, &jwt, "alice").await;

    let attachment_uuid = db
        .attachments()
        .create(
            user_id,
            b"encrypted_image_data",
            "image_iv",
            b"encrypted_thumbnail",
            "thumb_iv",
            1,
        )
        .await
        .unwrap();

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/attachments/{}/thumbnail", attachment_uuid))
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

    assert!(json["encrypted_thumbnail"].as_str().is_some());
    assert_eq!(json["iv"], "thumb_iv");
}

#[tokio::test]
async fn test_attachment_not_found() {
    let (app, db, jwt) = create_test_app().await;
    let (_, token) = create_authenticated_user(&db, &jwt, "alice").await;

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/attachments/00000000-0000-0000-0000-000000000000")
                .header("cookie", auth_cookie(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_cannot_access_other_users_attachment() {
    let (app, db, jwt) = create_test_app().await;
    let (alice_id, _alice_token) = create_authenticated_user(&db, &jwt, "alice").await;
    let (_, bob_token) = create_authenticated_user(&db, &jwt, "bob").await;

    // Alice creates an attachment
    let attachment_uuid = db
        .attachments()
        .create(alice_id, b"alice_image", "iv1", b"alice_thumb", "iv2", 1)
        .await
        .unwrap();

    // Bob tries to access Alice's attachment
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/attachments/{}", attachment_uuid))
                .header("cookie", auth_cookie(&bob_token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_update_refs_adds_attachment_to_post() {
    let (app, db, jwt) = create_test_app().await;
    let (user_id, token) = create_authenticated_user(&db, &jwt, "alice").await;

    // Create a post
    let post_uuid = db
        .posts()
        .create(
            user_id,
            Some("Test"),
            false,
            None,
            "content",
            false,
            None,
            None,
        )
        .await
        .unwrap();

    // Create an attachment
    let attachment_uuid = db
        .attachments()
        .create(user_id, b"img", "iv1", b"thumb", "iv2", 1)
        .await
        .unwrap();

    // Update post with attachment_uuids to add attachment
    let response = app
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/posts/{}", post_uuid))
                .header("content-type", "application/json")
                .header("cookie", auth_cookie(&token))
                .body(Body::from(format!(
                    r#"{{"content": "content", "attachment_uuids": ["{}"]}}"#,
                    attachment_uuid
                )))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    // Verify attachment ref count is now 1
    let attachment = db
        .attachments()
        .get_by_uuid(&attachment_uuid, user_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(attachment.reference_count, 1);
}

#[tokio::test]
async fn test_update_refs_removes_attachment_from_post() {
    let (app, db, jwt) = create_test_app().await;
    let (user_id, token) = create_authenticated_user(&db, &jwt, "alice").await;

    // Create a post
    let post_uuid = db
        .posts()
        .create(
            user_id,
            Some("Test"),
            false,
            None,
            "content",
            false,
            None,
            None,
        )
        .await
        .unwrap();
    let post = db
        .posts()
        .get_by_uuid(&post_uuid, user_id)
        .await
        .unwrap()
        .unwrap();

    // Create an attachment and add it to the post
    let attachment_uuid = db
        .attachments()
        .create(user_id, b"img", "iv1", b"thumb", "iv2", 1)
        .await
        .unwrap();

    db.attachments()
        .update_post_attachments(post.id, user_id, &[attachment_uuid.clone()])
        .await
        .unwrap();

    // Verify ref count is 1
    let attachment = db
        .attachments()
        .get_by_uuid(&attachment_uuid, user_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(attachment.reference_count, 1);

    // Update post with empty attachment_uuids to remove attachment
    let response = app
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/posts/{}", post_uuid))
                .header("content-type", "application/json")
                .header("cookie", auth_cookie(&token))
                .body(Body::from(
                    r#"{"content": "content", "attachment_uuids": []}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    // Attachment should be deleted (ref count went to 0)
    let attachment = db
        .attachments()
        .get_by_uuid(&attachment_uuid, user_id)
        .await
        .unwrap();
    assert!(attachment.is_none());
}

#[tokio::test]
async fn test_delete_post_removes_attachment_refs() {
    let (app, db, jwt) = create_test_app().await;
    let (user_id, token) = create_authenticated_user(&db, &jwt, "alice").await;

    // Create a post
    let post_uuid = db
        .posts()
        .create(
            user_id,
            Some("Test"),
            false,
            None,
            "content",
            false,
            None,
            None,
        )
        .await
        .unwrap();
    let post = db
        .posts()
        .get_by_uuid(&post_uuid, user_id)
        .await
        .unwrap()
        .unwrap();

    // Create an attachment and add it to the post
    let attachment_uuid = db
        .attachments()
        .create(user_id, b"img", "iv1", b"thumb", "iv2", 1)
        .await
        .unwrap();

    db.attachments()
        .update_post_attachments(post.id, user_id, &[attachment_uuid.clone()])
        .await
        .unwrap();

    // Delete the post via API
    let response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/posts/{}", post_uuid))
                .header("cookie", auth_cookie(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    // Attachment should be deleted (ref count went to 0)
    let attachment = db
        .attachments()
        .get_by_uuid(&attachment_uuid, user_id)
        .await
        .unwrap();
    assert!(attachment.is_none());
}

#[tokio::test]
async fn test_attachment_shared_between_posts() {
    let (app, db, jwt) = create_test_app().await;
    let (user_id, token) = create_authenticated_user(&db, &jwt, "alice").await;

    // Create two posts
    let post1_uuid = db
        .posts()
        .create(
            user_id,
            Some("Post 1"),
            false,
            None,
            "c1",
            false,
            None,
            None,
        )
        .await
        .unwrap();
    let post1 = db
        .posts()
        .get_by_uuid(&post1_uuid, user_id)
        .await
        .unwrap()
        .unwrap();

    let post2_uuid = db
        .posts()
        .create(
            user_id,
            Some("Post 2"),
            false,
            None,
            "c2",
            false,
            None,
            None,
        )
        .await
        .unwrap();
    let post2 = db
        .posts()
        .get_by_uuid(&post2_uuid, user_id)
        .await
        .unwrap()
        .unwrap();

    // Create an attachment
    let attachment_uuid = db
        .attachments()
        .create(user_id, b"img", "iv1", b"thumb", "iv2", 1)
        .await
        .unwrap();

    // Add attachment to both posts
    db.attachments()
        .update_post_attachments(post1.id, user_id, &[attachment_uuid.clone()])
        .await
        .unwrap();
    db.attachments()
        .update_post_attachments(post2.id, user_id, &[attachment_uuid.clone()])
        .await
        .unwrap();

    // Verify ref count is 2
    let attachment = db
        .attachments()
        .get_by_uuid(&attachment_uuid, user_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(attachment.reference_count, 2);

    // Delete post1 via API
    let response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/posts/{}", post1_uuid))
                .header("cookie", auth_cookie(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    // Attachment should still exist with ref count 1
    let attachment = db
        .attachments()
        .get_by_uuid(&attachment_uuid, user_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(attachment.reference_count, 1);
}

#[tokio::test]
async fn test_unauthenticated_upload_denied() {
    let (app, _, _) = create_test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/attachments")
                .header("content-type", "application/json")
                .body(Body::from(create_test_attachment_body()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_unauthenticated_get_denied() {
    let (app, _, _) = create_test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/attachments/some-uuid")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_upload_with_invalid_base64() {
    let (app, db, jwt) = create_test_app().await;
    let (_, token) = create_authenticated_user(&db, &jwt, "alice").await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/attachments")
                .header("content-type", "application/json")
                .header("cookie", auth_cookie(&token))
                .body(Body::from(
                    r#"{
                        "encrypted_image": "not-valid-base64!!!",
                        "encrypted_image_iv": "iv1",
                        "encrypted_thumbnail": "also-invalid",
                        "encrypted_thumbnail_iv": "iv2",
                        "encryption_version": 1
                    }"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}
