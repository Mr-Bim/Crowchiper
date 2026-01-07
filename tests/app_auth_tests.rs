//! Tests for app asset JWT authentication.

mod common;

use common::{runtime, setup};
use crowchiper::db::UserRole;
use crowchiper::jwt::JwtConfig;
use jsonwebtoken;

/// Helper to create a test server and get base URLs
async fn setup_with_jwt() -> (common::TestContext, JwtConfig) {
    let ctx = setup().await;
    let jwt = JwtConfig::new(b"test-jwt-secret-for-testing");
    (ctx, jwt)
}

/// Get the app base URL (protected assets)
fn app_url(ctx: &common::TestContext) -> String {
    // Replace /login with /fiery-sparrow in base_url
    ctx.base_url.replace("/login", env!("CONFIG_APP_ASSETS"))
}

#[test]
fn test_app_redirects_to_login_without_token() {
    runtime().block_on(async {
        let (ctx, _jwt) = setup_with_jwt().await;
        let app_base = app_url(&ctx);

        // Navigate to login first to clear any cookies from previous tests
        ctx.goto("/index.html").await;
        ctx.page
            .evaluate(
                "document.cookie = 'auth_token=; path=/; expires=Thu, 01 Jan 1970 00:00:00 GMT'",
            )
            .await
            .expect("Failed to clear cookie");

        // Try to access app without token - should redirect to login
        ctx.page
            .goto(&format!("{}/index.html", app_base))
            .await
            .expect("Failed to navigate");

        // Wait for redirect to complete
        ctx.wait_for("window.location.href.includes('/login')", 5000)
            .await
            .expect("Should redirect to login");

        ctx.teardown().await;
    });
}

#[test]
fn test_app_accessible_with_valid_token() {
    runtime().block_on(async {
        let (ctx, jwt) = setup_with_jwt().await;
        let app_base = app_url(&ctx);

        // Generate a valid token
        let token = jwt
            .generate_token("test-uuid", "testuser", UserRole::User)
            .expect("Failed to generate token");

        // First navigate to login page so we can set cookies on the domain
        ctx.goto("/index.html").await;

        // Set the cookie
        ctx.page
            .evaluate(format!("document.cookie = 'auth_token={}; path=/'", token))
            .await
            .expect("Failed to set cookie");

        // Now navigate to app
        ctx.page
            .goto(&format!("{}/index.html", app_base))
            .await
            .expect("Failed to navigate");

        // Should stay on app page (not redirected)
        let url: String = ctx.eval("window.location.href").await;
        assert!(
            url.contains(env!("CONFIG_APP_ASSETS")),
            "Should stay on app page, got: {}",
            url
        );

        // Verify the page title
        let title = ctx
            .page
            .get_title()
            .await
            .expect("Failed to get title")
            .unwrap_or_default();
        assert_eq!(title, "Crowchiper");

        ctx.teardown().await;
    });
}

#[test]
fn test_app_redirects_with_invalid_token() {
    runtime().block_on(async {
        let (ctx, _jwt) = setup_with_jwt().await;
        let app_base = app_url(&ctx);

        // First navigate to login page so we can set cookies
        ctx.goto("/index.html").await;

        // Set an invalid token
        ctx.page
            .evaluate("document.cookie = 'auth_token=invalid-token; path=/'")
            .await
            .expect("Failed to set cookie");

        // Navigate to app
        ctx.page
            .goto(&format!("{}/index.html", app_base))
            .await
            .expect("Failed to navigate");

        // Should redirect to login
        let url: String = ctx.eval("window.location.href").await;
        assert!(
            url.contains("/login"),
            "Should redirect to login with invalid token, got: {}",
            url
        );

        ctx.teardown().await;
    });
}

#[test]
fn test_app_redirects_with_expired_token() {
    runtime().block_on(async {
        let (ctx, _jwt) = setup_with_jwt().await;
        let app_base = app_url(&ctx);

        // Use a fresh page to avoid cookie contamination from other tests
        let page = ctx.new_page().await;

        // Create an expired token by directly constructing claims with exp in the past
        use std::time::{SystemTime, UNIX_EPOCH};
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let claims = crowchiper::jwt::Claims {
            sub: "test-uuid".to_string(),
            username: "testuser".to_string(),
            role: UserRole::User,
            iat: now - 100,
            exp: now - 50, // Expired 50 seconds ago
        };

        let token = jsonwebtoken::encode(
            &jsonwebtoken::Header::default(),
            &claims,
            &jsonwebtoken::EncodingKey::from_secret(b"test-jwt-secret-for-testing"),
        )
        .expect("Failed to encode token");

        // First navigate to login page so we can set cookies on the domain
        page.goto(&ctx.base_url)
            .await
            .expect("Failed to navigate to login");

        // Set the expired token
        page.evaluate(format!("document.cookie = 'auth_token={}; path=/'", token))
            .await
            .expect("Failed to set cookie");

        // Navigate to app
        page.goto(&format!("{}/index.html", app_base))
            .await
            .expect("Failed to navigate");

        // Check that we were redirected to login
        let url: String = page
            .evaluate("window.location.href")
            .await
            .expect("Failed to get URL")
            .into_value()
            .expect("Failed to parse URL");

        assert!(
            url.contains("/login"),
            "Should redirect to login with expired token, got: {}",
            url
        );

        ctx.teardown().await;
    });
}

#[test]
fn test_app_root_protected() {
    runtime().block_on(async {
        let (ctx, _jwt) = setup_with_jwt().await;
        let app_base = app_url(&ctx);

        // Try to access app root without token
        ctx.page.goto(&app_base).await.expect("Failed to navigate");

        // Wait for redirect to complete
        ctx.wait_for("window.location.href.includes('/login')", 5000)
            .await
            .expect("App root should redirect to login");

        ctx.teardown().await;
    });
}

#[test]
fn test_login_assets_accessible_without_token() {
    runtime().block_on(async {
        let (ctx, _jwt) = setup_with_jwt().await;

        // Login page should be accessible without token
        ctx.goto("/index.html").await;

        let title = ctx
            .page
            .get_title()
            .await
            .expect("Failed to get title")
            .unwrap_or_default();
        assert_eq!(title, "Crowchiper");

        // Register page should also be accessible
        ctx.goto("/register.html").await;

        let title = ctx
            .page
            .get_title()
            .await
            .expect("Failed to get title")
            .unwrap_or_default();
        assert_eq!(title, "Register - Crowchiper");

        ctx.teardown().await;
    });
}

#[test]
fn test_root_redirects_to_login() {
    runtime().block_on(async {
        let (ctx, _jwt) = setup_with_jwt().await;

        // Get base URL without /login path
        let root_url = ctx.base_url.replace("/login", "");

        ctx.page.goto(&root_url).await.expect("Failed to navigate");

        // Should redirect to login
        let url: String = ctx.eval("window.location.href").await;
        assert!(
            url.contains("/login"),
            "Root should redirect to login, got: {}",
            url
        );

        ctx.teardown().await;
    });
}

#[test]
fn test_admin_token_can_access_app() {
    runtime().block_on(async {
        let (ctx, jwt) = setup_with_jwt().await;
        let app_base = app_url(&ctx);

        // Generate an admin token
        let token = jwt
            .generate_token("admin-uuid", "admin", UserRole::Admin)
            .expect("Failed to generate token");

        // First navigate to login page so we can set cookies
        ctx.goto("/index.html").await;

        // Set the cookie
        ctx.page
            .evaluate(format!("document.cookie = 'auth_token={}; path=/'", token))
            .await
            .expect("Failed to set cookie");

        // Navigate to app
        ctx.page
            .goto(&format!("{}/index.html", app_base))
            .await
            .expect("Failed to navigate");

        // Should stay on app page
        let url: String = ctx.eval("window.location.href").await;
        assert!(
            url.contains(env!("CONFIG_APP_ASSETS")),
            "Admin should access app, got: {}",
            url
        );

        ctx.teardown().await;
    });
}

#[test]
fn test_app_with_base_path_redirects_to_login() {
    runtime().block_on(async {
        let ctx = common::setup_with_base(Some("/myapp")).await;

        // Get app URL with base path
        let app_base = ctx.base_url.replace("/login", env!("CONFIG_APP_ASSETS"));

        // Navigate to login first to clear any cookies from previous tests
        ctx.goto("/index.html").await;
        ctx.page
            .evaluate(
                "document.cookie = 'auth_token=; path=/; expires=Thu, 01 Jan 1970 00:00:00 GMT'",
            )
            .await
            .expect("Failed to clear cookie");

        // Try to access app without token
        ctx.page
            .goto(&format!("{}/index.html", app_base))
            .await
            .expect("Failed to navigate");

        // Wait for redirect to complete
        ctx.wait_for("window.location.href.includes('/login')", 5000)
            .await
            .expect("Should redirect to login with base path");

        ctx.teardown().await;
    });
}

#[test]
fn test_app_with_base_path_accessible_with_token() {
    runtime().block_on(async {
        let ctx = common::setup_with_base(Some("/crowchiper")).await;
        let jwt = JwtConfig::new(b"test-jwt-secret-for-testing");

        let app_base = ctx.base_url.replace("/login", env!("CONFIG_APP_ASSETS"));

        // Generate a valid token
        let token = jwt
            .generate_token("test-uuid", "testuser", UserRole::User)
            .expect("Failed to generate token");

        // Navigate to login first to set cookie
        ctx.goto("/index.html").await;

        // Set the cookie
        ctx.page
            .evaluate(format!("document.cookie = 'auth_token={}; path=/'", token))
            .await
            .expect("Failed to set cookie");

        // Navigate to app
        ctx.page
            .goto(&format!("{}/index.html", app_base))
            .await
            .expect("Failed to navigate");

        // Should stay on app page
        let url: String = ctx.eval("window.location.href").await;
        assert!(
            url.contains(env!("CONFIG_APP_ASSETS")),
            "Should access app with base path, got: {}",
            url
        );

        ctx.teardown().await;
    });
}
