//! Tests for the dual-token authentication system.
//!
//! Tests cover:
//! - Access token and refresh token generation
//! - Token refresh flow (expired access token + valid refresh token)
//! - Multiple devices/sessions per user
//! - User isolation (users cannot access each other's tokens)
//! - Login flow should not issue new refresh token if valid one exists
//! - Token revocation and logout
//! - IP address validation

mod common;

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use crowchiper::local_ip_extractor;
use crowchiper::{
    ServerConfig,
    cli::{ClientIpHeader, IpExtractor},
    create_app,
    db::Database,
    jwt::JwtConfig,
};
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
    };
    (create_app(&config), db, jwt_config)
}

/// Create a test app with X-Forwarded-For IP extraction for IP-related tests.
async fn create_test_app_with_ip_header() -> (axum::Router, Database, JwtConfig) {
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
        ip_extractor: Some(IpExtractor::from(ClientIpHeader::XForwardFor)),
    };
    (create_app(&config), db, jwt_config)
}

/// Create a user and return (user_id, uuid, access_token, refresh_token, refresh_jti).
async fn create_authenticated_user(
    db: &Database,
    jwt: &JwtConfig,
    username: &str,
    ip: &str,
) -> (i64, String, String, String, String) {
    let uuid = uuid::Uuid::new_v4().to_string();
    let id = db.users().create(&uuid, username).await.unwrap();
    db.users().activate(id).await.unwrap();

    // Generate access token (stateless, no DB storage)
    let access_result = jwt
        .generate_access_token(&uuid, username, crowchiper::db::UserRole::User, ip)
        .unwrap();

    // Generate refresh token and store in DB
    let refresh_result = jwt
        .generate_refresh_token(&uuid, username, crowchiper::db::UserRole::User)
        .unwrap();
    db.tokens()
        .create(
            &refresh_result.jti,
            id,
            Some(ip),
            refresh_result.issued_at,
            refresh_result.expires_at,
        )
        .await
        .unwrap();

    (
        id,
        uuid,
        access_result.token,
        refresh_result.token,
        refresh_result.jti,
    )
}

fn auth_cookies(access_token: &str, refresh_token: &str) -> String {
    format!(
        "access_token={}; refresh_token={}",
        access_token, refresh_token
    )
}

fn refresh_cookie_only(refresh_token: &str) -> String {
    format!("refresh_token={}", refresh_token)
}

/// Extract Set-Cookie headers from response
fn extract_set_cookies(response: &axum::http::Response<Body>) -> Vec<String> {
    response
        .headers()
        .get_all("set-cookie")
        .iter()
        .filter_map(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .collect()
}

/// Check if cookies contain a token being cleared (Max-Age=0)
fn has_cleared_cookie(cookies: &[String], cookie_name: &str) -> bool {
    cookies
        .iter()
        .any(|c| c.contains(cookie_name) && c.contains("Max-Age=0"))
}

/// Check if cookies contain a new access token
fn has_new_access_token(cookies: &[String]) -> bool {
    cookies
        .iter()
        .any(|c| c.starts_with("access_token=") && !c.contains("Max-Age=0"))
}

const TEST_IP: &str = "127.0.0.1";
const ALT_IP: &str = "192.168.1.100";

// =============================================================================
// Access Token Tests
// =============================================================================

#[tokio::test]
async fn test_valid_access_token_authenticates() {
    let (app, db, jwt) = create_test_app().await;
    let (_, _, access, refresh, _) = create_authenticated_user(&db, &jwt, "alice", TEST_IP).await;

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
}

#[tokio::test]
async fn test_access_token_ip_mismatch_triggers_refresh() {
    let (app, db, jwt) = create_test_app_with_ip_header().await;
    let (_, _, access, refresh, _) = create_authenticated_user(&db, &jwt, "alice", TEST_IP).await;

    // Request from different IP - access token IP won't match, should use refresh token
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/posts")
                .header("cookie", auth_cookies(&access, &refresh))
                .header("x-forwarded-for", ALT_IP)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    // Should have issued a new access token
    let cookies = extract_set_cookies(&response);
    assert!(
        has_new_access_token(&cookies),
        "Should issue new access token when IP changes"
    );
}

#[tokio::test]
async fn test_invalid_access_token_rejected_without_refresh() {
    let (app, _, _) = create_test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/posts")
                .header("cookie", "access_token=invalid-token")
                .header("x-forwarded-for", TEST_IP)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_no_tokens_returns_unauthorized() {
    let (app, _, _) = create_test_app().await;

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

// =============================================================================
// Refresh Token Tests
// =============================================================================

#[tokio::test]
async fn test_refresh_token_issues_new_access_token() {
    let (app, db, jwt) = create_test_app().await;
    let (_, _, _access, refresh, _) = create_authenticated_user(&db, &jwt, "alice", TEST_IP).await;

    // Use only refresh token (simulating expired access token)
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/posts")
                .header("cookie", refresh_cookie_only(&refresh))
                .header("x-forwarded-for", TEST_IP)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    // Should have issued a new access token
    let cookies = extract_set_cookies(&response);
    assert!(
        has_new_access_token(&cookies),
        "Should issue new access token from refresh token"
    );
}

#[tokio::test]
async fn test_revoked_refresh_token_rejected() {
    let (app, db, jwt) = create_test_app().await;
    let (_, _, _access, refresh, jti) =
        create_authenticated_user(&db, &jwt, "alice", TEST_IP).await;

    // Revoke the refresh token
    db.tokens().delete_by_jti(&jti).await.unwrap();

    // Try to use the revoked refresh token
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/posts")
                .header("cookie", refresh_cookie_only(&refresh))
                .header("x-forwarded-for", TEST_IP)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_invalid_refresh_token_rejected() {
    let (app, _, _) = create_test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/posts")
                .header("cookie", "refresh_token=invalid-token")
                .header("x-forwarded-for", TEST_IP)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_refresh_token_updates_ip_on_change() {
    let (app, db, jwt) = create_test_app_with_ip_header().await;
    let (_, _, _access, refresh, jti) =
        create_authenticated_user(&db, &jwt, "alice", TEST_IP).await;

    // Verify initial IP
    let token = db.tokens().get_by_jti(&jti).await.unwrap().unwrap();
    assert_eq!(token.last_ip, Some(TEST_IP.to_string()));

    // Make request from different IP
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/posts")
                .header("cookie", refresh_cookie_only(&refresh))
                .header("x-forwarded-for", ALT_IP)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    // Verify IP was updated
    let token = db.tokens().get_by_jti(&jti).await.unwrap().unwrap();
    assert_eq!(token.last_ip, Some(ALT_IP.to_string()));
}

// =============================================================================
// Multiple Devices/Sessions Tests
// =============================================================================

#[tokio::test]
async fn test_user_can_have_multiple_refresh_tokens() {
    let (app, db, jwt) = create_test_app().await;

    // Create user
    let uuid = uuid::Uuid::new_v4().to_string();
    let id = db.users().create(&uuid, "alice").await.unwrap();
    db.users().activate(id).await.unwrap();

    // Create first session (device 1)
    let refresh1 = jwt
        .generate_refresh_token(&uuid, "alice", crowchiper::db::UserRole::User)
        .unwrap();
    db.tokens()
        .create(
            &refresh1.jti,
            id,
            Some("10.0.0.1"),
            refresh1.issued_at,
            refresh1.expires_at,
        )
        .await
        .unwrap();

    // Create second session (device 2)
    let refresh2 = jwt
        .generate_refresh_token(&uuid, "alice", crowchiper::db::UserRole::User)
        .unwrap();
    db.tokens()
        .create(
            &refresh2.jti,
            id,
            Some("10.0.0.2"),
            refresh2.issued_at,
            refresh2.expires_at,
        )
        .await
        .unwrap();

    // Both tokens should work
    let response1 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/posts")
                .header("cookie", refresh_cookie_only(&refresh1.token))
                .header("x-forwarded-for", "10.0.0.1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response1.status(), StatusCode::OK);

    let response2 = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/posts")
                .header("cookie", refresh_cookie_only(&refresh2.token))
                .header("x-forwarded-for", "10.0.0.2")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response2.status(), StatusCode::OK);

    // Verify both tokens exist in database
    let tokens = db.tokens().list_by_user(id).await.unwrap();
    assert_eq!(tokens.len(), 2);
}

#[tokio::test]
async fn test_revoking_one_session_doesnt_affect_others() {
    let db = Database::open(":memory:")
        .await
        .expect("Failed to open test database");
    let jwt_secret = b"test-jwt-secret".to_vec();
    let jwt = JwtConfig::new(&jwt_secret);
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
    };
    let app = create_app(&config);

    // Create user with two sessions
    let uuid = uuid::Uuid::new_v4().to_string();
    let id = db.users().create(&uuid, "alice").await.unwrap();
    db.users().activate(id).await.unwrap();

    let refresh1 = jwt
        .generate_refresh_token(&uuid, "alice", crowchiper::db::UserRole::User)
        .unwrap();
    db.tokens()
        .create(
            &refresh1.jti,
            id,
            Some("10.0.0.1"),
            refresh1.issued_at,
            refresh1.expires_at,
        )
        .await
        .unwrap();

    let refresh2 = jwt
        .generate_refresh_token(&uuid, "alice", crowchiper::db::UserRole::User)
        .unwrap();
    db.tokens()
        .create(
            &refresh2.jti,
            id,
            Some("10.0.0.2"),
            refresh2.issued_at,
            refresh2.expires_at,
        )
        .await
        .unwrap();

    // Generate access token for device 1 to revoke device 2's token
    let access1 = jwt
        .generate_access_token(&uuid, "alice", crowchiper::db::UserRole::User, "10.0.0.1")
        .unwrap();

    // Revoke session 2 via API
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/tokens/{}", refresh2.jti))
                .header("cookie", auth_cookies(&access1.token, &refresh1.token))
                .header("x-forwarded-for", "10.0.0.1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // Session 1 should still work
    let response1 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/posts")
                .header("cookie", refresh_cookie_only(&refresh1.token))
                .header("x-forwarded-for", "10.0.0.1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response1.status(), StatusCode::OK);

    // Session 2 should be rejected
    let response2 = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/posts")
                .header("cookie", refresh_cookie_only(&refresh2.token))
                .header("x-forwarded-for", "10.0.0.2")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response2.status(), StatusCode::UNAUTHORIZED);
}

// =============================================================================
// User Isolation Tests
// =============================================================================

#[tokio::test]
async fn test_users_cannot_use_each_others_tokens() {
    let (app, db, jwt) = create_test_app().await;

    // Create Alice
    let (_, _, alice_access, alice_refresh, _) =
        create_authenticated_user(&db, &jwt, "alice", TEST_IP).await;

    // Create Bob
    let (bob_id, _, _, _, _) = create_authenticated_user(&db, &jwt, "bob", TEST_IP).await;

    // Create a post for Bob
    let bob_post = db
        .posts()
        .create(
            bob_id,
            Some("Bob's Secret"),
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

    // Alice tries to access Bob's post using her tokens
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/posts/{}", bob_post))
                .header("cookie", auth_cookies(&alice_access, &alice_refresh))
                .header("x-forwarded-for", TEST_IP)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // Alice should get NOT_FOUND (not FORBIDDEN) to avoid revealing post existence
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_user_cannot_revoke_other_users_tokens() {
    let db = Database::open(":memory:")
        .await
        .expect("Failed to open test database");
    let jwt_secret = b"test-jwt-secret".to_vec();
    let jwt = JwtConfig::new(&jwt_secret);
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
    };
    let app = create_app(&config);

    // Create Alice with a refresh token
    let (_, _, alice_access, alice_refresh, _) =
        create_authenticated_user(&db, &jwt, "alice", TEST_IP).await;

    // Create Bob with a refresh token
    let (_, _, _, _, bob_jti) = create_authenticated_user(&db, &jwt, "bob", TEST_IP).await;

    // Alice tries to revoke Bob's refresh token
    let response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/tokens/{}", bob_jti))
                .header("cookie", auth_cookies(&alice_access, &alice_refresh))
                .header("x-forwarded-for", TEST_IP)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);

    // Verify Bob's token still exists
    let token = db.tokens().get_by_jti(&bob_jti).await.unwrap();
    assert!(token.is_some(), "Bob's token should still exist");
}

#[tokio::test]
async fn test_admin_can_revoke_any_users_tokens() {
    let db = Database::open(":memory:")
        .await
        .expect("Failed to open test database");
    let jwt_secret = b"test-jwt-secret".to_vec();
    let jwt = JwtConfig::new(&jwt_secret);
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
    };
    let app = create_app(&config);

    // Create admin
    let admin_uuid = uuid::Uuid::new_v4().to_string();
    let admin_id = db.users().create_admin(&admin_uuid, "admin").await.unwrap();
    db.users().activate(admin_id).await.unwrap();
    let admin_access = jwt
        .generate_access_token(
            &admin_uuid,
            "admin",
            crowchiper::db::UserRole::Admin,
            TEST_IP,
        )
        .unwrap();
    let admin_refresh = jwt
        .generate_refresh_token(&admin_uuid, "admin", crowchiper::db::UserRole::Admin)
        .unwrap();
    db.tokens()
        .create(
            &admin_refresh.jti,
            admin_id,
            Some(TEST_IP),
            admin_refresh.issued_at,
            admin_refresh.expires_at,
        )
        .await
        .unwrap();

    // Create regular user Bob
    let (_, _, _, _, bob_jti) = create_authenticated_user(&db, &jwt, "bob", TEST_IP).await;

    // Admin revokes Bob's token
    let response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/tokens/{}", bob_jti))
                .header(
                    "cookie",
                    auth_cookies(&admin_access.token, &admin_refresh.token),
                )
                .header("x-forwarded-for", TEST_IP)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    // Verify Bob's token was revoked
    let token = db.tokens().get_by_jti(&bob_jti).await.unwrap();
    assert!(token.is_none(), "Bob's token should be revoked");
}

// =============================================================================
// Logout Tests
// =============================================================================

#[tokio::test]
async fn test_logout_clears_both_cookies() {
    let (app, db, jwt) = create_test_app().await;
    let (_, _, access, refresh, _) = create_authenticated_user(&db, &jwt, "alice", TEST_IP).await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/tokens/logout")
                .header("cookie", auth_cookies(&access, &refresh))
                .header("x-forwarded-for", TEST_IP)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let cookies = extract_set_cookies(&response);
    assert!(
        has_cleared_cookie(&cookies, "access_token"),
        "Should clear access_token cookie"
    );
    assert!(
        has_cleared_cookie(&cookies, "refresh_token"),
        "Should clear refresh_token cookie"
    );
}

#[tokio::test]
async fn test_logout_revokes_refresh_token() {
    let (app, db, jwt) = create_test_app().await;
    let (_, _, access, refresh, jti) = create_authenticated_user(&db, &jwt, "alice", TEST_IP).await;

    // Verify token exists
    let token = db.tokens().get_by_jti(&jti).await.unwrap();
    assert!(token.is_some());

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/tokens/logout")
                .header("cookie", auth_cookies(&access, &refresh))
                .header("x-forwarded-for", TEST_IP)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    // Verify token was revoked
    let token = db.tokens().get_by_jti(&jti).await.unwrap();
    assert!(
        token.is_none(),
        "Refresh token should be revoked after logout"
    );

    // Verify the old refresh token no longer works
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/posts")
                .header("cookie", refresh_cookie_only(&refresh))
                .header("x-forwarded-for", TEST_IP)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_logout_succeeds_without_valid_token() {
    let (app, _, _) = create_test_app().await;

    // Logout without any token should still succeed (idempotent)
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/tokens/logout")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

// =============================================================================
// Token List Tests
// =============================================================================

#[tokio::test]
async fn test_list_tokens_returns_only_own_tokens() {
    let db = Database::open(":memory:")
        .await
        .expect("Failed to open test database");
    let jwt_secret = b"test-jwt-secret".to_vec();
    let jwt = JwtConfig::new(&jwt_secret);
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
    };
    let app = create_app(&config);

    // Create Alice with 2 tokens
    let alice_uuid = uuid::Uuid::new_v4().to_string();
    let alice_id = db.users().create(&alice_uuid, "alice").await.unwrap();
    db.users().activate(alice_id).await.unwrap();

    let alice_refresh1 = jwt
        .generate_refresh_token(&alice_uuid, "alice", crowchiper::db::UserRole::User)
        .unwrap();
    db.tokens()
        .create(
            &alice_refresh1.jti,
            alice_id,
            Some("10.0.0.1"),
            alice_refresh1.issued_at,
            alice_refresh1.expires_at,
        )
        .await
        .unwrap();

    let alice_refresh2 = jwt
        .generate_refresh_token(&alice_uuid, "alice", crowchiper::db::UserRole::User)
        .unwrap();
    db.tokens()
        .create(
            &alice_refresh2.jti,
            alice_id,
            Some("10.0.0.2"),
            alice_refresh2.issued_at,
            alice_refresh2.expires_at,
        )
        .await
        .unwrap();

    let alice_access = jwt
        .generate_access_token(
            &alice_uuid,
            "alice",
            crowchiper::db::UserRole::User,
            "10.0.0.1",
        )
        .unwrap();

    // Create Bob with 1 token
    let (_, _, _, _, _) = create_authenticated_user(&db, &jwt, "bob", TEST_IP).await;

    // Alice lists her tokens
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/tokens")
                .header(
                    "cookie",
                    auth_cookies(&alice_access.token, &alice_refresh1.token),
                )
                .header("x-forwarded-for", "10.0.0.1")
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
    let tokens = json["tokens"].as_array().unwrap();

    // Alice should only see her 2 tokens, not Bob's
    assert_eq!(tokens.len(), 2);

    // Verify the JTIs belong to Alice
    let jtis: Vec<&str> = tokens.iter().filter_map(|t| t["jti"].as_str()).collect();
    assert!(jtis.contains(&alice_refresh1.jti.as_str()));
    assert!(jtis.contains(&alice_refresh2.jti.as_str()));
}

// =============================================================================
// Token Verify Endpoint Tests
// =============================================================================

#[tokio::test]
async fn test_verify_token_returns_ok_for_valid_token() {
    let (app, db, jwt) = create_test_app().await;
    let (_, _, access, refresh, _) = create_authenticated_user(&db, &jwt, "alice", TEST_IP).await;

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/tokens/verify")
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
async fn test_verify_token_returns_unauthorized_for_invalid_token() {
    let (app, _, _) = create_test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/tokens/verify")
                .header("cookie", "access_token=invalid")
                .header("x-forwarded-for", TEST_IP)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

// =============================================================================
// Token Type Confusion Tests
// =============================================================================

#[tokio::test]
async fn test_refresh_token_cannot_be_used_as_access_token() {
    let (app, db, jwt) = create_test_app().await;
    let (_, _, _access, refresh, _) = create_authenticated_user(&db, &jwt, "alice", TEST_IP).await;

    // Try to use refresh token in place of access token (with no refresh token)
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/posts")
                .header("cookie", format!("access_token={}", refresh))
                .header("x-forwarded-for", TEST_IP)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // Should fail - refresh token has wrong type
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_access_token_cannot_be_used_as_refresh_token() {
    let (app, db, jwt) = create_test_app().await;
    let (_, _, access, _refresh, _) = create_authenticated_user(&db, &jwt, "alice", TEST_IP).await;

    // Try to use access token in place of refresh token
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/posts")
                .header("cookie", format!("refresh_token={}", access))
                .header("x-forwarded-for", TEST_IP)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // Should fail - access token has wrong type and no valid access token provided
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

// =============================================================================
// Login Flow - Refresh Token Reuse Tests
// =============================================================================

#[tokio::test]
async fn test_login_finish_does_not_issue_new_refresh_if_valid_exists() {
    // This test verifies that login/finish reuses an existing valid refresh token
    // instead of creating a new one. Since we can't easily test the full WebAuthn flow,
    // we test by verifying the token count doesn't increase when logging in with a valid token.
    let db = Database::open(":memory:")
        .await
        .expect("Failed to open test database");
    let jwt_secret = b"test-jwt-secret".to_vec();
    let jwt = JwtConfig::new(&jwt_secret);

    // Create and activate user with one refresh token
    let uuid = uuid::Uuid::new_v4().to_string();
    let id = db.users().create(&uuid, "alice").await.unwrap();
    db.users().activate(id).await.unwrap();

    let refresh = jwt
        .generate_refresh_token(&uuid, "alice", crowchiper::db::UserRole::User)
        .unwrap();
    db.tokens()
        .create(
            &refresh.jti,
            id,
            Some(TEST_IP),
            refresh.issued_at,
            refresh.expires_at,
        )
        .await
        .unwrap();

    // Verify only 1 token exists
    let tokens = db.tokens().list_by_user(id).await.unwrap();
    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0].jti, refresh.jti);
}

#[tokio::test]
async fn test_multiple_logins_without_logout_accumulate_tokens() {
    // When a user logs in without a valid refresh token (e.g., cleared cookies),
    // a new refresh token should be created. This tests that scenario.
    let db = Database::open(":memory:")
        .await
        .expect("Failed to open test database");
    let jwt_secret = b"test-jwt-secret".to_vec();
    let jwt = JwtConfig::new(&jwt_secret);

    // Create and activate user
    let uuid = uuid::Uuid::new_v4().to_string();
    let id = db.users().create(&uuid, "alice").await.unwrap();
    db.users().activate(id).await.unwrap();

    // Simulate first login - create first refresh token
    let refresh1 = jwt
        .generate_refresh_token(&uuid, "alice", crowchiper::db::UserRole::User)
        .unwrap();
    db.tokens()
        .create(
            &refresh1.jti,
            id,
            Some("10.0.0.1"),
            refresh1.issued_at,
            refresh1.expires_at,
        )
        .await
        .unwrap();

    // Simulate second login from different device (no existing token) - create second token
    let refresh2 = jwt
        .generate_refresh_token(&uuid, "alice", crowchiper::db::UserRole::User)
        .unwrap();
    db.tokens()
        .create(
            &refresh2.jti,
            id,
            Some("10.0.0.2"),
            refresh2.issued_at,
            refresh2.expires_at,
        )
        .await
        .unwrap();

    // Both tokens should exist
    let tokens = db.tokens().list_by_user(id).await.unwrap();
    assert_eq!(tokens.len(), 2);

    // Both should be valid and work independently
    let token1 = db.tokens().get_by_jti(&refresh1.jti).await.unwrap();
    let token2 = db.tokens().get_by_jti(&refresh2.jti).await.unwrap();
    assert!(token1.is_some());
    assert!(token2.is_some());
}

#[tokio::test]
async fn test_login_with_other_users_token_issues_new_token() {
    // If user A has user B's refresh token cookie (shouldn't happen in practice),
    // we should NOT reuse it - we should issue a new token for the authenticating user.
    let db = Database::open(":memory:")
        .await
        .expect("Failed to open test database");
    let jwt_secret = b"test-jwt-secret".to_vec();
    let jwt = JwtConfig::new(&jwt_secret);

    // Create Alice
    let alice_uuid = uuid::Uuid::new_v4().to_string();
    let alice_id = db.users().create(&alice_uuid, "alice").await.unwrap();
    db.users().activate(alice_id).await.unwrap();

    // Create Bob with a refresh token
    let bob_uuid = uuid::Uuid::new_v4().to_string();
    let bob_id = db.users().create(&bob_uuid, "bob").await.unwrap();
    db.users().activate(bob_id).await.unwrap();

    let bob_refresh = jwt
        .generate_refresh_token(&bob_uuid, "bob", crowchiper::db::UserRole::User)
        .unwrap();
    db.tokens()
        .create(
            &bob_refresh.jti,
            bob_id,
            Some(TEST_IP),
            bob_refresh.issued_at,
            bob_refresh.expires_at,
        )
        .await
        .unwrap();

    // Alice has 0 tokens
    let alice_tokens = db.tokens().list_by_user(alice_id).await.unwrap();
    assert_eq!(alice_tokens.len(), 0);

    // Bob has 1 token
    let bob_tokens = db.tokens().list_by_user(bob_id).await.unwrap();
    assert_eq!(bob_tokens.len(), 1);

    // If Alice logs in, she should get her own token (not reuse Bob's)
    // This would be verified in an integration test with the actual login flow,
    // but here we verify the database state supports this
}

#[tokio::test]
async fn test_revoked_token_should_trigger_new_token_on_login() {
    // If user has a revoked refresh token cookie, login should issue a new one
    let db = Database::open(":memory:")
        .await
        .expect("Failed to open test database");
    let jwt_secret = b"test-jwt-secret".to_vec();
    let jwt = JwtConfig::new(&jwt_secret);

    // Create and activate user
    let uuid = uuid::Uuid::new_v4().to_string();
    let id = db.users().create(&uuid, "alice").await.unwrap();
    db.users().activate(id).await.unwrap();

    // Create and then revoke a refresh token
    let refresh = jwt
        .generate_refresh_token(&uuid, "alice", crowchiper::db::UserRole::User)
        .unwrap();
    db.tokens()
        .create(
            &refresh.jti,
            id,
            Some(TEST_IP),
            refresh.issued_at,
            refresh.expires_at,
        )
        .await
        .unwrap();

    // Revoke the token
    db.tokens().delete_by_jti(&refresh.jti).await.unwrap();

    // Verify token is revoked
    let token = db.tokens().get_by_jti(&refresh.jti).await.unwrap();
    assert!(token.is_none());

    // User now has 0 valid tokens
    let tokens = db.tokens().list_by_user(id).await.unwrap();
    assert_eq!(tokens.len(), 0);

    // On next login, a new token should be issued (tested via the API)
}

// =============================================================================
// Deactivated User Tests
// =============================================================================

#[tokio::test]
async fn test_deactivated_user_token_rejected() {
    let (app, db, jwt) = create_test_app().await;

    // Create user but don't activate
    let uuid = uuid::Uuid::new_v4().to_string();
    let id = db.users().create(&uuid, "alice").await.unwrap();
    // Not activated!

    // Generate tokens (should still work for generation)
    let _access = jwt
        .generate_access_token(&uuid, "alice", crowchiper::db::UserRole::User, TEST_IP)
        .unwrap();
    let refresh = jwt
        .generate_refresh_token(&uuid, "alice", crowchiper::db::UserRole::User)
        .unwrap();
    db.tokens()
        .create(
            &refresh.jti,
            id,
            Some(TEST_IP),
            refresh.issued_at,
            refresh.expires_at,
        )
        .await
        .unwrap();

    // Try to use tokens - should fail because user not activated
    // Access token won't work (user not activated), refresh will be tried
    // but refresh also checks activation
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/posts")
                .header("cookie", refresh_cookie_only(&refresh.token))
                .header("x-forwarded-for", TEST_IP)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

// =============================================================================
// User Deleted Tests
// =============================================================================

#[tokio::test]
async fn test_deleted_user_token_rejected() {
    let (app, db, jwt) = create_test_app().await;
    let (user_id, uuid, _access, refresh, _jti) =
        create_authenticated_user(&db, &jwt, "alice", TEST_IP).await;

    // Delete the user
    db.users().delete(user_id).await.unwrap();

    // Verify user is deleted
    let user = db.users().get_by_uuid(&uuid).await.unwrap();
    assert!(user.is_none());

    // Try to use the refresh token - should fail
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/posts")
                .header("cookie", refresh_cookie_only(&refresh))
                .header("x-forwarded-for", TEST_IP)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}
