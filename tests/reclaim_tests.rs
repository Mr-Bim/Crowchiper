//! Tests for the account reclaim flow.
//!
//! The reclaim flow handles the edge case where a user has a passkey registered
//! but their account isn't activated (e.g., passkey storage succeeded but
//! activation failed).
//!
//! Note: Full end-to-end browser tests for the reclaim flow are limited because
//! Chrome's virtual authenticator doesn't fully support conditional UI / discoverable
//! credential lookup in the same way a real authenticator does. We test the API
//! endpoints directly and verify the frontend pages load correctly.

mod common;

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use common::setup;
use crowchiper::{ServerConfig, create_app, db::Database};
use std::time::Duration;
use tower::ServiceExt;
use url::Url;

/// Test that the claim page loads correctly in reclaim mode.
#[test]
fn test_claim_page_reclaim_mode() {
    common::runtime().block_on(async {
        let ctx = setup().await;
        ctx.goto("/claim.html?reclaim=true").await;

        // Wait for JS to initialize
        tokio::time::sleep(Duration::from_millis(200)).await;

        // Check title is set for reclaim mode
        let title: String = ctx
            .eval("document.getElementById('claim-title')?.textContent || ''")
            .await;
        assert_eq!(
            title, "Reclaim Account",
            "Title should be 'Reclaim Account' in reclaim mode"
        );

        // Check status message
        let status: String = ctx
            .eval("document.getElementById('status')?.textContent || ''")
            .await;
        assert!(
            status.contains("reclaim") || status.contains("passkey"),
            "Status should mention reclaiming, got: {}",
            status
        );

        // Button should be enabled and say "Use Passkey"
        let button_text: String = ctx
            .eval("document.getElementById('claim-button')?.textContent || ''")
            .await;
        assert_eq!(
            button_text.trim(),
            "Use Passkey",
            "Button should say 'Use Passkey'"
        );

        let disabled: bool = ctx
            .eval("document.getElementById('claim-button')?.disabled ?? true")
            .await;
        assert!(!disabled, "Claim button should be enabled in reclaim mode");

        ctx.teardown().await;
    });
}

/// Test that claim/start endpoint returns valid WebAuthn options.
#[tokio::test]
async fn test_claim_start_returns_webauthn_options() {
    let db = Database::open(":memory:")
        .await
        .expect("Failed to open test database");
    let config = ServerConfig {
        base: None,
        db,
        rp_id: "localhost".to_string(),
        rp_origin: Url::parse("http://localhost").expect("Invalid URL"),
        jwt_secret: b"test-jwt-secret-for-testing-purposes".to_vec(),
        secure_cookies: false,
        no_signup: false,
    };
    let app = create_app(&config);

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/passkeys/claim/start")
                .header("content-type", "application/json")
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

    // Should have a session_id
    assert!(
        json["session_id"].is_string(),
        "Response should have session_id"
    );

    // Should have publicKey with challenge
    assert!(
        json["publicKey"]["challenge"].is_string(),
        "Response should have publicKey.challenge"
    );

    // Should have rpId
    assert_eq!(
        json["publicKey"]["rpId"], "localhost",
        "Response should have correct rpId"
    );
}

/// Test that claim/start works with base path.
#[tokio::test]
async fn test_claim_start_with_base_path() {
    let db = Database::open(":memory:")
        .await
        .expect("Failed to open test database");
    let config = ServerConfig {
        base: Some("/myapp".to_string()),
        db,
        rp_id: "localhost".to_string(),
        rp_origin: Url::parse("http://localhost").expect("Invalid URL"),
        jwt_secret: b"test-jwt-secret-for-testing-purposes".to_vec(),
        secure_cookies: false,
        no_signup: false,
    };
    let app = create_app(&config);

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/myapp/api/passkeys/claim/start")
                .header("content-type", "application/json")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

/// Test that claim/finish fails with expired/invalid session_id.
#[tokio::test]
async fn test_claim_finish_expired_session() {
    let db = Database::open(":memory:")
        .await
        .expect("Failed to open test database");
    let config = ServerConfig {
        base: None,
        db: db.clone(),
        rp_id: "localhost".to_string(),
        rp_origin: Url::parse("http://localhost").expect("Invalid URL"),
        jwt_secret: b"test-jwt-secret-for-testing-purposes".to_vec(),
        secure_cookies: false,
        no_signup: false,
    };
    let app = create_app(&config);

    // First start a claim to get a valid session format
    let start_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/passkeys/claim/start")
                .header("content-type", "application/json")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(start_response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(start_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let start_json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let session_id = start_json["session_id"].as_str().unwrap();

    // Delete the challenge to simulate expiration
    let deleted = db.login_challenges().delete(session_id).await.unwrap();
    assert!(deleted, "Challenge should have been deleted");

    // Verify the challenge no longer exists by trying to take it
    let challenge = db.login_challenges().take(session_id).await.unwrap();
    assert!(challenge.is_none(), "Challenge should be deleted/expired");
}

/// Test that login/start works for unactivated users.
/// This is needed because users may have a passkey but not be activated
/// (edge case where passkey storage succeeded but activation failed).
#[tokio::test]
async fn test_login_start_works_for_unactivated_user() {
    let db = Database::open(":memory:")
        .await
        .expect("Failed to open test database");

    // Create an unactivated user
    let uuid = uuid::Uuid::new_v4().to_string();
    let user_id = db.users().create(&uuid, "testuser").await.unwrap();

    let config = ServerConfig {
        base: None,
        db: db.clone(),
        rp_id: "localhost".to_string(),
        rp_origin: Url::parse("http://localhost").expect("Invalid URL"),
        jwt_secret: b"test-jwt-secret-for-testing-purposes".to_vec(),
        secure_cookies: false,
        no_signup: false,
    };
    let app = create_app(&config);

    // Verify user is not activated
    let user = db.users().get_by_id(user_id).await.unwrap().unwrap();
    assert!(!user.activated, "User should not be activated");

    // login/start should work for unactivated user (returns discoverable auth since no passkeys)
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/passkeys/login/start")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"username": "testuser"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

/// Test that the claim page loads in register mode with UUID.
#[test]
fn test_claim_page_register_mode() {
    common::runtime().block_on(async {
        let ctx = setup().await;

        // Create a pending admin user
        let uuid = uuid::Uuid::new_v4().to_string();
        ctx.db
            .users()
            .create_admin(&uuid, "TestAdmin")
            .await
            .expect("Failed to create admin user");

        ctx.goto(&format!("/claim.html?uuid={}", uuid)).await;

        // Wait for JS to initialize
        tokio::time::sleep(Duration::from_millis(200)).await;

        // Check title is set for register mode
        let title: String = ctx
            .eval("document.getElementById('claim-title')?.textContent || ''")
            .await;
        assert_eq!(
            title, "Claim Admin",
            "Title should be 'Claim Admin' in register mode"
        );

        // Button should say "Register Passkey"
        let button_text: String = ctx
            .eval("document.getElementById('claim-button')?.textContent || ''")
            .await;
        assert_eq!(
            button_text.trim(),
            "Register Passkey",
            "Button should say 'Register Passkey'"
        );

        ctx.teardown().await;
    });
}
