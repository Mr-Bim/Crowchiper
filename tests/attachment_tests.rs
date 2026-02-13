//! Tests for the Attachments API: upload, post linking, reference counting, and cleanup.

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

    let access_result = jwt
        .generate_access_token(&uuid, username, crowchiper::db::UserRole::User, "127.0.0.1")
        .unwrap();

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

const TEST_IP: &str = "127.0.0.1";

/// Build a multipart/form-data body for uploading an unencrypted attachment.
fn build_multipart_body(
    image: &[u8],
    thumb_sm: &[u8],
    thumb_md: Option<&[u8]>,
    thumb_lg: Option<&[u8]>,
) -> (String, Vec<u8>) {
    let boundary = "----TestBoundary12345";
    let mut body = Vec::new();

    // image field
    body.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
    body.extend_from_slice(
        b"Content-Disposition: form-data; name=\"image\"; filename=\"image.webp\"\r\n",
    );
    body.extend_from_slice(b"Content-Type: application/octet-stream\r\n\r\n");
    body.extend_from_slice(image);
    body.extend_from_slice(b"\r\n");

    // image_iv field (empty for unencrypted)
    body.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
    body.extend_from_slice(b"Content-Disposition: form-data; name=\"image_iv\"\r\n\r\n");
    body.extend_from_slice(b"\r\n");

    // thumb_sm field
    body.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
    body.extend_from_slice(
        b"Content-Disposition: form-data; name=\"thumb_sm\"; filename=\"thumb_sm.webp\"\r\n",
    );
    body.extend_from_slice(b"Content-Type: application/octet-stream\r\n\r\n");
    body.extend_from_slice(thumb_sm);
    body.extend_from_slice(b"\r\n");

    // thumb_sm_iv field
    body.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
    body.extend_from_slice(b"Content-Disposition: form-data; name=\"thumb_sm_iv\"\r\n\r\n");
    body.extend_from_slice(b"\r\n");

    // thumb_md field (optional)
    if let Some(md) = thumb_md {
        body.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
        body.extend_from_slice(
            b"Content-Disposition: form-data; name=\"thumb_md\"; filename=\"thumb_md.webp\"\r\n",
        );
        body.extend_from_slice(b"Content-Type: application/octet-stream\r\n\r\n");
        body.extend_from_slice(md);
        body.extend_from_slice(b"\r\n");

        body.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
        body.extend_from_slice(b"Content-Disposition: form-data; name=\"thumb_md_iv\"\r\n\r\n");
        body.extend_from_slice(b"\r\n");
    }

    // thumb_lg field (optional)
    if let Some(lg) = thumb_lg {
        body.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
        body.extend_from_slice(
            b"Content-Disposition: form-data; name=\"thumb_lg\"; filename=\"thumb_lg.webp\"\r\n",
        );
        body.extend_from_slice(b"Content-Type: application/octet-stream\r\n\r\n");
        body.extend_from_slice(lg);
        body.extend_from_slice(b"\r\n");

        body.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
        body.extend_from_slice(b"Content-Disposition: form-data; name=\"thumb_lg_iv\"\r\n\r\n");
        body.extend_from_slice(b"\r\n");
    }

    // encryption_version field (0 = unencrypted)
    body.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
    body.extend_from_slice(b"Content-Disposition: form-data; name=\"encryption_version\"\r\n\r\n");
    body.extend_from_slice(b"0\r\n");

    // End boundary
    body.extend_from_slice(format!("--{}--\r\n", boundary).as_bytes());

    let content_type = format!("multipart/form-data; boundary={}", boundary);
    (content_type, body)
}

/// Upload an attachment via the API and return its UUID.
async fn upload_attachment(
    app: &axum::Router,
    access: &str,
    refresh: &str,
    image: &[u8],
) -> String {
    let (content_type, body) = build_multipart_body(image, b"thumb_sm_data", None, None);

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/attachments")
                .header("content-type", &content_type)
                .header("cookie", auth_cookies(access, refresh))
                .header("x-forwarded-for", TEST_IP)
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
    json["uuid"].as_str().unwrap().to_string()
}

// --- Tests ---

#[tokio::test]
async fn test_upload_unencrypted_attachment() {
    let (app, db, jwt) = create_test_app().await;
    let (user_id, access, refresh) = create_authenticated_user(&db, &jwt, "alice").await;

    let uuid = upload_attachment(&app, &access, &refresh, b"test_image_data").await;

    // Verify it was stored correctly
    let attachment = db
        .attachments()
        .get_by_uuid(&uuid, user_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(attachment.image_data, b"test_image_data");
    assert_eq!(attachment.encryption_version, 0);
    assert!(attachment.image_iv.is_none());
    assert_eq!(attachment.reference_count, 0);
}

#[tokio::test]
async fn test_get_attachment_via_api() {
    let (app, db, jwt) = create_test_app().await;
    let (_, access, refresh) = create_authenticated_user(&db, &jwt, "alice").await;

    let uuid = upload_attachment(&app, &access, &refresh, b"image_bytes_here").await;

    // Fetch the full image
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/attachments/{}", uuid))
                .header("cookie", auth_cookies(&access, &refresh))
                .header("x-forwarded-for", TEST_IP)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    // IV header should be empty for unencrypted
    let iv = response.headers().get("x-encryption-iv").unwrap();
    assert_eq!(iv, "");

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    assert_eq!(&body[..], b"image_bytes_here");
}

#[tokio::test]
async fn test_get_thumbnail_via_api() {
    let (app, db, jwt) = create_test_app().await;
    let (_, access, refresh) = create_authenticated_user(&db, &jwt, "alice").await;

    let (content_type, body) = build_multipart_body(
        b"img",
        b"small_thumb",
        Some(b"med_thumb"),
        Some(b"large_thumb"),
    );

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/attachments")
                .header("content-type", &content_type)
                .header("cookie", auth_cookies(&access, &refresh))
                .header("x-forwarded-for", TEST_IP)
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);
    let resp_body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&resp_body).unwrap();
    let uuid = json["uuid"].as_str().unwrap();

    // Fetch each thumbnail size
    for (size, expected) in [
        ("sm", b"small_thumb" as &[u8]),
        ("md", b"med_thumb"),
        ("lg", b"large_thumb"),
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!("/api/attachments/{}/thumbnail/{}", uuid, size))
                    .header("cookie", auth_cookies(&access, &refresh))
                    .header("x-forwarded-for", TEST_IP)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(
            response.status(),
            StatusCode::OK,
            "Failed for size {}",
            size
        );
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        assert_eq!(&body[..], expected, "Wrong data for size {}", size);
    }
}

#[tokio::test]
async fn test_cannot_access_other_users_attachment() {
    let (app, db, jwt) = create_test_app().await;
    let (_, alice_access, alice_refresh) = create_authenticated_user(&db, &jwt, "alice").await;
    let (_, bob_access, bob_refresh) = create_authenticated_user(&db, &jwt, "bob").await;

    // Alice uploads an attachment
    let uuid = upload_attachment(&app, &alice_access, &alice_refresh, b"alice_secret").await;

    // Bob tries to fetch it
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/attachments/{}", uuid))
                .header("cookie", auth_cookies(&bob_access, &bob_refresh))
                .header("x-forwarded-for", TEST_IP)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    // Bob tries to fetch thumbnail
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/attachments/{}/thumbnail/sm", uuid))
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
async fn test_upload_attachment_link_to_post_then_update_removes() {
    let (app, db, jwt) = create_test_app().await;
    let (user_id, access, refresh) = create_authenticated_user(&db, &jwt, "alice").await;

    // Upload 3 attachments
    let uuid1 = upload_attachment(&app, &access, &refresh, b"image_1").await;
    let uuid2 = upload_attachment(&app, &access, &refresh, b"image_2").await;
    let uuid3 = upload_attachment(&app, &access, &refresh, b"image_3").await;

    // Create a post
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/posts")
                .header("content-type", "application/json")
                .header("cookie", auth_cookies(&access, &refresh))
                .header("x-forwarded-for", TEST_IP)
                .body(Body::from(r#"{"content": "initial"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let post_uuid = json["uuid"].as_str().unwrap().to_string();

    // Update post with all 3 attachments
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
                    r#"{{"content": "with images", "attachment_uuids": ["{}", "{}", "{}"]}}"#,
                    uuid1, uuid2, uuid3
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    // Verify all 3 have ref_count = 1
    for uuid in [&uuid1, &uuid2, &uuid3] {
        let att = db
            .attachments()
            .get_by_uuid(uuid, user_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(att.reference_count, 1, "ref_count for {}", uuid);
    }

    // Update post: remove uuid2, keep uuid1 and uuid3
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
                    r#"{{"content": "two images", "attachment_uuids": ["{}", "{}"]}}"#,
                    uuid1, uuid3
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    // uuid2 should be deleted (ref_count was 1, decremented to 0)
    let att2 = db.attachments().get_by_uuid(&uuid2, user_id).await.unwrap();
    assert!(att2.is_none(), "uuid2 should be deleted after removal");

    // uuid1 and uuid3 should still exist with ref_count = 1
    for uuid in [&uuid1, &uuid3] {
        let att = db
            .attachments()
            .get_by_uuid(uuid, user_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(att.reference_count, 1, "ref_count for {}", uuid);
    }

    // Update post: remove all attachments
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/posts/{}", post_uuid))
                .header("content-type", "application/json")
                .header("cookie", auth_cookies(&access, &refresh))
                .header("x-forwarded-for", TEST_IP)
                .body(Body::from(
                    r#"{"content": "no images", "attachment_uuids": []}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    // All attachments should be deleted
    for uuid in [&uuid1, &uuid3] {
        let att = db.attachments().get_by_uuid(uuid, user_id).await.unwrap();
        assert!(att.is_none(), "{} should be deleted", uuid);
    }
}

#[tokio::test]
async fn test_delete_post_cleans_up_attachments() {
    let (app, db, jwt) = create_test_app().await;
    let (user_id, access, refresh) = create_authenticated_user(&db, &jwt, "alice").await;

    // Upload 2 attachments
    let uuid1 = upload_attachment(&app, &access, &refresh, b"image_a").await;
    let uuid2 = upload_attachment(&app, &access, &refresh, b"image_b").await;

    // Create a post and link both attachments
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/posts")
                .header("content-type", "application/json")
                .header("cookie", auth_cookies(&access, &refresh))
                .header("x-forwarded-for", TEST_IP)
                .body(Body::from(r#"{"content": "test"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let post_uuid = json["uuid"].as_str().unwrap().to_string();

    // Link attachments to the post
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
                    r#"{{"content": "images", "attachment_uuids": ["{}", "{}"]}}"#,
                    uuid1, uuid2
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    // Verify attachments exist
    assert!(
        db.attachments()
            .get_by_uuid(&uuid1, user_id)
            .await
            .unwrap()
            .is_some()
    );
    assert!(
        db.attachments()
            .get_by_uuid(&uuid2, user_id)
            .await
            .unwrap()
            .is_some()
    );

    // Delete the post
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

    // Both attachments should be deleted (ref_count was 1, went to 0)
    assert!(
        db.attachments()
            .get_by_uuid(&uuid1, user_id)
            .await
            .unwrap()
            .is_none(),
        "uuid1 should be deleted after post deletion"
    );
    assert!(
        db.attachments()
            .get_by_uuid(&uuid2, user_id)
            .await
            .unwrap()
            .is_none(),
        "uuid2 should be deleted after post deletion"
    );
}

#[tokio::test]
async fn test_shared_attachment_between_posts() {
    let (app, db, jwt) = create_test_app().await;
    let (user_id, access, refresh) = create_authenticated_user(&db, &jwt, "alice").await;

    // Upload 1 attachment
    let uuid = upload_attachment(&app, &access, &refresh, b"shared_image").await;

    // Create 2 posts
    let mut post_uuids = Vec::new();
    for _ in 0..2 {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/posts")
                    .header("content-type", "application/json")
                    .header("cookie", auth_cookies(&access, &refresh))
                    .header("x-forwarded-for", TEST_IP)
                    .body(Body::from(r#"{"content": "post"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        post_uuids.push(json["uuid"].as_str().unwrap().to_string());
    }

    // Link the same attachment to both posts
    for post_uuid in &post_uuids {
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
                        r#"{{"content": "image", "attachment_uuids": ["{}"]}}"#,
                        uuid
                    )))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
    }

    // Attachment should have ref_count = 2
    let att = db
        .attachments()
        .get_by_uuid(&uuid, user_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(att.reference_count, 2);

    // Delete first post
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/posts/{}", post_uuids[0]))
                .header("cookie", auth_cookies(&access, &refresh))
                .header("x-forwarded-for", TEST_IP)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // Attachment should still exist with ref_count = 1
    let att = db
        .attachments()
        .get_by_uuid(&uuid, user_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(att.reference_count, 1);

    // Delete second post
    let response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/posts/{}", post_uuids[1]))
                .header("cookie", auth_cookies(&access, &refresh))
                .header("x-forwarded-for", TEST_IP)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // Attachment should now be deleted (ref_count went to 0)
    assert!(
        db.attachments()
            .get_by_uuid(&uuid, user_id)
            .await
            .unwrap()
            .is_none(),
        "Attachment should be deleted when last referencing post is deleted"
    );
}

#[tokio::test]
async fn test_update_without_attachment_uuids_preserves_refs() {
    let (app, db, jwt) = create_test_app().await;
    let (user_id, access, refresh) = create_authenticated_user(&db, &jwt, "alice").await;

    // Upload and link an attachment
    let uuid = upload_attachment(&app, &access, &refresh, b"keep_me").await;

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/posts")
                .header("content-type", "application/json")
                .header("cookie", auth_cookies(&access, &refresh))
                .header("x-forwarded-for", TEST_IP)
                .body(Body::from(r#"{"content": "post"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let post_uuid = json["uuid"].as_str().unwrap().to_string();

    // Link attachment
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
                    r#"{{"content": "with image", "attachment_uuids": ["{}"]}}"#,
                    uuid
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    // Update post content only (no attachment_uuids field)
    let response = app
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/posts/{}", post_uuid))
                .header("content-type", "application/json")
                .header("cookie", auth_cookies(&access, &refresh))
                .header("x-forwarded-for", TEST_IP)
                .body(Body::from(r#"{"content": "updated text"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    // Attachment should still exist with ref_count = 1
    let att = db
        .attachments()
        .get_by_uuid(&uuid, user_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        att.reference_count, 1,
        "Attachment should be preserved when attachment_uuids is omitted"
    );
}

#[tokio::test]
async fn test_upload_requires_authentication() {
    let (app, _, _) = create_test_app().await;

    let (content_type, body) = build_multipart_body(b"img", b"thumb", None, None);

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/attachments")
                .header("content-type", &content_type)
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_delete_parent_post_cleans_up_child_attachments() {
    let (app, db, jwt) = create_test_app().await;
    let (user_id, access, refresh) = create_authenticated_user(&db, &jwt, "alice").await;

    // Upload attachments for parent and child
    let parent_att = upload_attachment(&app, &access, &refresh, b"parent_img").await;
    let child_att = upload_attachment(&app, &access, &refresh, b"child_img").await;

    // Create parent post
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/posts")
                .header("content-type", "application/json")
                .header("cookie", auth_cookies(&access, &refresh))
                .header("x-forwarded-for", TEST_IP)
                .body(Body::from(r#"{"content": "parent"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let parent_uuid = json["uuid"].as_str().unwrap().to_string();

    // Create child post under parent
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
                    r#"{{"content": "child", "parent_id": "{}"}}"#,
                    parent_uuid
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let child_uuid = json["uuid"].as_str().unwrap().to_string();

    // Link attachments to their respective posts
    for (post_uuid, att_uuid) in [(&parent_uuid, &parent_att), (&child_uuid, &child_att)] {
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
                        r#"{{"content": "img", "attachment_uuids": ["{}"]}}"#,
                        att_uuid
                    )))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
    }

    // Delete parent (cascades to child)
    let response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/posts/{}", parent_uuid))
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
    assert_eq!(json["children_deleted"], 1);

    // Both attachments should be deleted
    assert!(
        db.attachments()
            .get_by_uuid(&parent_att, user_id)
            .await
            .unwrap()
            .is_none(),
        "Parent attachment should be cleaned up"
    );
    assert!(
        db.attachments()
            .get_by_uuid(&child_att, user_id)
            .await
            .unwrap()
            .is_none(),
        "Child attachment should be cleaned up"
    );
}
