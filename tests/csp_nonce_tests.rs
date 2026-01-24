//! Tests for CSP nonce functionality.
//!
//! Verifies that when --csp-nonce is enabled, HTML responses include a random nonce
//! in the Content-Security-Policy header's script-src directive.

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use crowchiper::{ServerConfig, create_app, db::Database, jwt::JwtConfig};
use tower::ServiceExt;
use url::Url;

/// App assets path from config.json (set at compile time)
const APP_PATH: &str = env!("CONFIG_APP_ASSETS");

/// Create a test app with csp_nonce enabled.
async fn create_app_with_nonce() -> (axum::Router, Database, JwtConfig) {
    let db = Database::open(":memory:")
        .await
        .expect("Failed to open test database");
    let jwt_secret = b"test-jwt-secret-for-csp-nonce-tests".to_vec();
    let jwt_config = JwtConfig::new(&jwt_secret);
    let config = ServerConfig {
        base: None,
        db: db.clone(),
        rp_id: "localhost".to_string(),
        rp_origin: Url::parse("http://localhost").expect("Invalid URL"),
        jwt_secret,
        secure_cookies: false,
        no_signup: false,
        csp_nonce: true,
    };
    (create_app(&config), db, jwt_config)
}

/// Create a test app without csp_nonce (default behavior).
async fn create_app_without_nonce() -> axum::Router {
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
    };
    create_app(&config)
}

/// Extract the nonce value from a CSP header, if present.
fn extract_nonce(csp: &str) -> Option<String> {
    // Look for 'nonce-XXX' pattern in the CSP header
    let nonce_prefix = "'nonce-";
    if let Some(start) = csp.find(nonce_prefix) {
        let after_prefix = &csp[start + nonce_prefix.len()..];
        if let Some(end) = after_prefix.find('\'') {
            return Some(after_prefix[..end].to_string());
        }
    }
    None
}

/// Verify that a CSP header contains a nonce in script-src.
fn assert_has_nonce(csp: &str) {
    assert!(
        csp.contains("script-src"),
        "CSP should contain script-src directive"
    );
    let nonce = extract_nonce(csp);
    assert!(nonce.is_some(), "CSP should contain a nonce: {}", csp);
    let nonce = nonce.unwrap();
    // Nonce should be base64 encoded (16 bytes = 24 chars with padding, or 22 without)
    assert!(
        nonce.len() >= 20 && nonce.len() <= 24,
        "Nonce should be ~22-24 chars (base64 of 16 bytes), got {} chars: {}",
        nonce.len(),
        nonce
    );
}

/// Verify that a CSP header does NOT contain a nonce.
fn assert_no_nonce(csp: &str) {
    let nonce = extract_nonce(csp);
    assert!(nonce.is_none(), "CSP should NOT contain a nonce: {}", csp);
}

// =============================================================================
// Tests with csp_nonce: true
// =============================================================================

#[tokio::test]
async fn test_login_index_has_nonce() {
    let (app, _, _) = create_app_with_nonce().await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/login/")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let csp = response
        .headers()
        .get("content-security-policy")
        .expect("Should have CSP header")
        .to_str()
        .unwrap();

    assert_has_nonce(csp);
}

#[tokio::test]
async fn test_login_register_has_nonce() {
    let (app, _, _) = create_app_with_nonce().await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/login/register.html")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let csp = response
        .headers()
        .get("content-security-policy")
        .expect("Should have CSP header")
        .to_str()
        .unwrap();

    assert_has_nonce(csp);
}

#[tokio::test]
async fn test_login_claim_has_nonce() {
    let (app, _, _) = create_app_with_nonce().await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/login/claim.html")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let csp = response
        .headers()
        .get("content-security-policy")
        .expect("Should have CSP header")
        .to_str()
        .unwrap();

    assert_has_nonce(csp);
}

#[tokio::test]
async fn test_app_index_has_nonce() {
    let (app, db, jwt) = create_app_with_nonce().await;

    // Create and activate a user to get a valid token
    let uuid = uuid::Uuid::new_v4().to_string();
    let id = db
        .users()
        .create(&uuid, "testuser")
        .await
        .expect("Failed to create user");
    db.users()
        .activate(id)
        .await
        .expect("Failed to activate user");

    let access_result = jwt
        .generate_access_token(
            &uuid,
            "testuser",
            crowchiper::db::UserRole::User,
            "127.0.0.1",
        )
        .unwrap();

    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("{}/", APP_PATH))
                .header("cookie", format!("access_token={}", access_result.token))
                .header("x-forwarded-for", "127.0.0.1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let csp = response
        .headers()
        .get("content-security-policy")
        .expect("Should have CSP header")
        .to_str()
        .unwrap();

    assert_has_nonce(csp);
}

#[tokio::test]
async fn test_nonces_are_different_per_request() {
    let (app, _, _) = create_app_with_nonce().await;

    // First request
    let response1 = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/login/")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let csp1 = response1
        .headers()
        .get("content-security-policy")
        .unwrap()
        .to_str()
        .unwrap();
    let nonce1 = extract_nonce(csp1).expect("Should have nonce");

    // Second request
    let response2 = app
        .oneshot(
            Request::builder()
                .uri("/login/")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let csp2 = response2
        .headers()
        .get("content-security-policy")
        .unwrap()
        .to_str()
        .unwrap();
    let nonce2 = extract_nonce(csp2).expect("Should have nonce");

    assert_ne!(nonce1, nonce2, "Each request should have a unique nonce");
}

// =============================================================================
// Tests with csp_nonce: false (default)
// =============================================================================

#[tokio::test]
async fn test_login_index_no_nonce_when_disabled() {
    let app = create_app_without_nonce().await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/login/")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let csp = response
        .headers()
        .get("content-security-policy")
        .expect("Should have CSP header")
        .to_str()
        .unwrap();

    assert_no_nonce(csp);
    // But it should still have script-src with hashes
    assert!(csp.contains("script-src"), "Should still have script-src");
    assert!(csp.contains("sha384-"), "Should have SHA384 hashes");
}

#[tokio::test]
async fn test_register_no_nonce_when_disabled() {
    let app = create_app_without_nonce().await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/login/register.html")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let csp = response
        .headers()
        .get("content-security-policy")
        .expect("Should have CSP header")
        .to_str()
        .unwrap();

    assert_no_nonce(csp);
}

#[tokio::test]
async fn test_claim_no_nonce_when_disabled() {
    let app = create_app_without_nonce().await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/login/claim.html")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let csp = response
        .headers()
        .get("content-security-policy")
        .expect("Should have CSP header")
        .to_str()
        .unwrap();

    assert_no_nonce(csp);
}
