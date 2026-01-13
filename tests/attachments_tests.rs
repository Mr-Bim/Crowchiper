//! Tests for the Attachments API.

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

/// Create a multipart form body for uploading an unencrypted attachment with all thumbnail sizes.
/// Uses encryption_version=0 and empty IVs for unencrypted uploads.
fn create_multipart_body() -> (String, Vec<u8>) {
    let boundary = "----TestBoundary12345";
    let image_data = b"fake image data";
    let thumb_sm_data = b"fake thumb sm";
    let thumb_md_data = b"fake thumb md";
    let thumb_lg_data = b"fake thumb lg";

    let mut body = Vec::new();

    // Image field (binary)
    body.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
    body.extend_from_slice(
        b"Content-Disposition: form-data; name=\"image\"; filename=\"image.bin\"\r\n",
    );
    body.extend_from_slice(b"Content-Type: application/octet-stream\r\n\r\n");
    body.extend_from_slice(image_data);
    body.extend_from_slice(b"\r\n");

    // Image IV field (empty for unencrypted)
    body.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
    body.extend_from_slice(b"Content-Disposition: form-data; name=\"image_iv\"\r\n\r\n");
    body.extend_from_slice(b"");
    body.extend_from_slice(b"\r\n");

    // Small thumbnail
    body.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
    body.extend_from_slice(
        b"Content-Disposition: form-data; name=\"thumb_sm\"; filename=\"thumb_sm.bin\"\r\n",
    );
    body.extend_from_slice(b"Content-Type: application/octet-stream\r\n\r\n");
    body.extend_from_slice(thumb_sm_data);
    body.extend_from_slice(b"\r\n");

    // Small thumbnail IV (empty for unencrypted)
    body.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
    body.extend_from_slice(b"Content-Disposition: form-data; name=\"thumb_sm_iv\"\r\n\r\n");
    body.extend_from_slice(b"");
    body.extend_from_slice(b"\r\n");

    // Medium thumbnail
    body.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
    body.extend_from_slice(
        b"Content-Disposition: form-data; name=\"thumb_md\"; filename=\"thumb_md.bin\"\r\n",
    );
    body.extend_from_slice(b"Content-Type: application/octet-stream\r\n\r\n");
    body.extend_from_slice(thumb_md_data);
    body.extend_from_slice(b"\r\n");

    // Medium thumbnail IV (empty for unencrypted)
    body.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
    body.extend_from_slice(b"Content-Disposition: form-data; name=\"thumb_md_iv\"\r\n\r\n");
    body.extend_from_slice(b"");
    body.extend_from_slice(b"\r\n");

    // Large thumbnail
    body.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
    body.extend_from_slice(
        b"Content-Disposition: form-data; name=\"thumb_lg\"; filename=\"thumb_lg.bin\"\r\n",
    );
    body.extend_from_slice(b"Content-Type: application/octet-stream\r\n\r\n");
    body.extend_from_slice(thumb_lg_data);
    body.extend_from_slice(b"\r\n");

    // Large thumbnail IV (empty for unencrypted)
    body.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
    body.extend_from_slice(b"Content-Disposition: form-data; name=\"thumb_lg_iv\"\r\n\r\n");
    body.extend_from_slice(b"");
    body.extend_from_slice(b"\r\n");

    // Encryption version field (0 = unencrypted)
    body.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
    body.extend_from_slice(b"Content-Disposition: form-data; name=\"encryption_version\"\r\n\r\n");
    body.extend_from_slice(b"0");
    body.extend_from_slice(b"\r\n");

    // End boundary
    body.extend_from_slice(format!("--{}--\r\n", boundary).as_bytes());

    let content_type = format!("multipart/form-data; boundary={}", boundary);
    (content_type, body)
}

/// Helper to create attachment input for database tests
fn create_db_attachment_input(
    user_id: i64,
) -> crowchiper::db::attachments::CreateAttachmentInput<'static> {
    crowchiper::db::attachments::CreateAttachmentInput {
        user_id,
        image_data: b"image_data_bytes",
        image_iv: Some("image_iv"),
        thumb_sm: b"thumb_sm_data",
        thumb_sm_iv: Some("thumb_sm_iv"),
        thumb_md: Some((b"thumb_md_data".as_slice(), Some("thumb_md_iv"))),
        thumb_lg: Some((b"thumb_lg_data".as_slice(), Some("thumb_lg_iv"))),
        encryption_version: 1,
    }
}

#[tokio::test]
async fn test_upload_attachment() {
    let (app, db, jwt) = create_test_app().await;
    let (_, token) = create_authenticated_user(&db, &jwt, "alice").await;

    let (content_type, body) = create_multipart_body();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/attachments")
                .header("content-type", content_type)
                .header("cookie", auth_cookie(&token))
                .body(Body::from(body))
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
    let input = create_db_attachment_input(user_id);
    let attachment_uuid = db.attachments().create(input).await.unwrap();

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

    // Check the IV is in the header
    let iv = response.headers().get("X-Encryption-IV").unwrap();
    assert_eq!(iv.to_str().unwrap(), "image_iv");

    // Check content type is binary
    let content_type = response.headers().get("content-type").unwrap();
    assert_eq!(content_type.to_str().unwrap(), "application/octet-stream");

    // Check the body is the raw image data
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    assert_eq!(&body[..], b"image_data_bytes");
}

#[tokio::test]
async fn test_get_thumbnails() {
    let (app, db, jwt) = create_test_app().await;
    let (user_id, token) = create_authenticated_user(&db, &jwt, "alice").await;

    let input = create_db_attachment_input(user_id);
    let attachment_uuid = db.attachments().create(input).await.unwrap();

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/attachments/{}/thumbnails", attachment_uuid))
                .header("cookie", auth_cookie(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    // Check content type is multipart
    let content_type = response
        .headers()
        .get("content-type")
        .unwrap()
        .to_str()
        .unwrap();
    assert!(content_type.starts_with("multipart/mixed"));

    // Check the body contains all thumbnail sizes
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body_str = String::from_utf8_lossy(&body);

    assert!(body_str.contains("X-Thumbnail-Size: sm"));
    assert!(body_str.contains("X-Thumbnail-Size: md"));
    assert!(body_str.contains("X-Thumbnail-Size: lg"));
    assert!(body_str.contains("thumb_sm_iv"));
    assert!(body_str.contains("thumb_md_iv"));
    assert!(body_str.contains("thumb_lg_iv"));
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
    let input = crowchiper::db::attachments::CreateAttachmentInput {
        user_id: alice_id,
        image_data: b"alice_image",
        image_iv: Some("iv1"),
        thumb_sm: b"alice_thumb",
        thumb_sm_iv: Some("iv2"),
        thumb_md: None,
        thumb_lg: None,
        encryption_version: 1,
    };
    let attachment_uuid = db.attachments().create(input).await.unwrap();

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
    let input = crowchiper::db::attachments::CreateAttachmentInput {
        user_id,
        image_data: b"img",
        image_iv: Some("iv1"),
        thumb_sm: b"thumb",
        thumb_sm_iv: Some("iv2"),
        thumb_md: None,
        thumb_lg: None,
        encryption_version: 1,
    };
    let attachment_uuid = db.attachments().create(input).await.unwrap();

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
    let input = crowchiper::db::attachments::CreateAttachmentInput {
        user_id,
        image_data: b"img",
        image_iv: Some("iv1"),
        thumb_sm: b"thumb",
        thumb_sm_iv: Some("iv2"),
        thumb_md: None,
        thumb_lg: None,
        encryption_version: 1,
    };
    let attachment_uuid = db.attachments().create(input).await.unwrap();

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
    let input = crowchiper::db::attachments::CreateAttachmentInput {
        user_id,
        image_data: b"img",
        image_iv: Some("iv1"),
        thumb_sm: b"thumb",
        thumb_sm_iv: Some("iv2"),
        thumb_md: None,
        thumb_lg: None,
        encryption_version: 1,
    };
    let attachment_uuid = db.attachments().create(input).await.unwrap();

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
    let input = crowchiper::db::attachments::CreateAttachmentInput {
        user_id,
        image_data: b"img",
        image_iv: Some("iv1"),
        thumb_sm: b"thumb",
        thumb_sm_iv: Some("iv2"),
        thumb_md: None,
        thumb_lg: None,
        encryption_version: 1,
    };
    let attachment_uuid = db.attachments().create(input).await.unwrap();

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

    let (content_type, body) = create_multipart_body();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/attachments")
                .header("content-type", content_type)
                .body(Body::from(body))
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
async fn test_upload_missing_required_fields() {
    let (app, db, jwt) = create_test_app().await;
    let (_, token) = create_authenticated_user(&db, &jwt, "alice").await;

    // Create a multipart body with missing fields
    let boundary = "----TestBoundary12345";
    let mut body = Vec::new();

    // Only include image field, missing others
    body.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
    body.extend_from_slice(
        b"Content-Disposition: form-data; name=\"image\"; filename=\"image.bin\"\r\n",
    );
    body.extend_from_slice(b"Content-Type: application/octet-stream\r\n\r\n");
    body.extend_from_slice(b"fake image data");
    body.extend_from_slice(b"\r\n");
    body.extend_from_slice(format!("--{}--\r\n", boundary).as_bytes());

    let content_type = format!("multipart/form-data; boundary={}", boundary);

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/attachments")
                .header("content-type", content_type)
                .header("cookie", auth_cookie(&token))
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}
