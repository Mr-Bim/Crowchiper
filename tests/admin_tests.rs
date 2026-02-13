mod common;

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
#[cfg(feature = "test-mode")]
use crowchiper::local_ip_extractor;
use crowchiper::{ServerConfig, create_app, db::Database, db::UserRole};
use tower::ServiceExt;
use url::Url;

async fn create_test_app() -> (axum::Router, Database) {
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
        ip_extractor: Some(local_ip_extractor()),
        plugin_manager: None,
    };
    (create_app(&config), db)
}

fn create_jwt() -> crowchiper::jwt::JwtConfig {
    crowchiper::jwt::JwtConfig::new(b"test-jwt-secret")
}

// --- Admin users endpoint tests ---

#[tokio::test]
async fn test_admin_users_requires_auth() {
    let (app, _db) = create_test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/admin/users")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_admin_users_requires_admin_role() {
    let (app, db) = create_test_app().await;
    let jwt = create_jwt();

    // Create a regular user
    let uuid = "00000000-0000-0000-0000-000000000001";
    let id = db.users().create(uuid, "alice").await.unwrap();
    db.users().activate(id).await.unwrap();

    let access = jwt
        .generate_access_token(uuid, "alice", UserRole::User, "127.0.0.1")
        .unwrap();

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/admin/users")
                .header("cookie", format!("access_token={}", access.token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_admin_users_succeeds_for_admin() {
    let (app, db) = create_test_app().await;
    let jwt = create_jwt();

    // Create a regular user
    let user_uuid = "00000000-0000-0000-0000-000000000001";
    let user_id = db.users().create(user_uuid, "alice").await.unwrap();
    db.users().activate(user_id).await.unwrap();

    // Create an admin
    let admin_uuid = "00000000-0000-0000-0000-000000000002";
    let admin_id = db.users().create_admin(admin_uuid, "admin").await.unwrap();
    db.users().activate(admin_id).await.unwrap();

    let access = jwt
        .generate_access_token(admin_uuid, "admin", UserRole::Admin, "127.0.0.1")
        .unwrap();

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/admin/users")
                .header("cookie", format!("access_token={}", access.token))
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

    let users = json.as_array().expect("Response should be an array");
    assert_eq!(users.len(), 2);

    // Check fields exist and no internal IDs are exposed
    for user in users {
        assert!(user.get("uuid").is_some(), "Should have uuid");
        assert!(user.get("username").is_some(), "Should have username");
        assert!(user.get("role").is_some(), "Should have role");
        assert!(user.get("activated").is_some(), "Should have activated");
        assert!(user.get("created_at").is_some(), "Should have created_at");
        assert!(user.get("id").is_none(), "Should NOT expose internal id");
    }
}

#[tokio::test]
async fn test_admin_users_no_internal_ids() {
    let (app, db) = create_test_app().await;
    let jwt = create_jwt();

    let admin_uuid = "00000000-0000-0000-0000-000000000001";
    let admin_id = db.users().create_admin(admin_uuid, "admin").await.unwrap();
    db.users().activate(admin_id).await.unwrap();

    let access = jwt
        .generate_access_token(admin_uuid, "admin", UserRole::Admin, "127.0.0.1")
        .unwrap();

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/admin/users")
                .header("cookie", format!("access_token={}", access.token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body_str = String::from_utf8_lossy(&body);

    // Ensure no "id" key at the top level of user objects
    let json: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();
    for user in &json {
        let obj = user.as_object().unwrap();
        // Only allowed keys
        let allowed = ["uuid", "username", "role", "activated", "created_at"];
        for key in obj.keys() {
            assert!(
                allowed.contains(&key.as_str()),
                "Unexpected field '{}' in response: {}",
                key,
                body_str
            );
        }
    }
}

// --- User settings endpoint tests ---

#[tokio::test]
async fn test_user_settings_returns_is_admin_true_for_admin() {
    let (app, db) = create_test_app().await;
    let jwt = create_jwt();

    let admin_uuid = "00000000-0000-0000-0000-000000000001";
    let admin_id = db.users().create_admin(admin_uuid, "admin").await.unwrap();
    db.users().activate(admin_id).await.unwrap();

    let access = jwt
        .generate_access_token(admin_uuid, "admin", UserRole::Admin, "127.0.0.1")
        .unwrap();

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/user/settings")
                .header("cookie", format!("access_token={}", access.token))
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

    assert_eq!(json["is_admin"], true);
    assert!(
        json["dashboard_path"].is_string(),
        "Admin should get dashboard_path"
    );
}

#[tokio::test]
async fn test_user_settings_returns_is_admin_false_for_regular_user() {
    let (app, db) = create_test_app().await;
    let jwt = create_jwt();

    let uuid = "00000000-0000-0000-0000-000000000001";
    let id = db.users().create(uuid, "alice").await.unwrap();
    db.users().activate(id).await.unwrap();

    let access = jwt
        .generate_access_token(uuid, "alice", UserRole::User, "127.0.0.1")
        .unwrap();

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/user/settings")
                .header("cookie", format!("access_token={}", access.token))
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

    assert_eq!(json["is_admin"], false);
    assert!(
        json["dashboard_path"].is_null(),
        "Regular user should NOT get dashboard_path"
    );
}

#[tokio::test]
async fn test_old_encryption_settings_endpoint_removed() {
    let (app, db) = create_test_app().await;
    let jwt = create_jwt();

    let uuid = "00000000-0000-0000-0000-000000000001";
    let id = db.users().create(uuid, "alice").await.unwrap();
    db.users().activate(id).await.unwrap();

    let access = jwt
        .generate_access_token(uuid, "alice", UserRole::User, "127.0.0.1")
        .unwrap();

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/encryption/settings")
                .header("cookie", format!("access_token={}", access.token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // The old endpoint should no longer exist
    assert_eq!(
        response.status(),
        StatusCode::NOT_FOUND,
        "Old /api/encryption/settings endpoint should be removed"
    );
}
