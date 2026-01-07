//! Tests for sidebar collapse functionality on small screens.

mod common;

use chromiumoxide::cdp::browser_protocol::emulation::SetDeviceMetricsOverrideParams;
use common::{runtime, setup};
use crowchiper::db::UserRole;
use crowchiper::jwt::JwtConfig;

/// Get the app base URL (protected assets)
fn app_url(ctx: &common::TestContext) -> String {
    ctx.base_url.replace("/login", env!("CONFIG_APP_ASSETS"))
}

#[test]
fn test_sidebar_toggle_collapses_on_small_screen() {
    runtime().block_on(async {
        let ctx = setup().await;
        let jwt = JwtConfig::new(b"test-jwt-secret-for-testing");
        let app_base = app_url(&ctx);

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

        // Set viewport to mobile size (triggers responsive behavior at <= 768px)
        ctx.page
            .execute(
                SetDeviceMetricsOverrideParams::builder()
                    .width(600)
                    .height(800)
                    .device_scale_factor(1.0)
                    .mobile(false)
                    .build()
                    .unwrap(),
            )
            .await
            .expect("Failed to set viewport");

        // Navigate to app
        ctx.page
            .goto(&format!("{}/index.html", app_base))
            .await
            .expect("Failed to navigate");

        // Wait for app to load
        ctx.wait_for("document.getElementById('sidebar') !== null", 5000)
            .await
            .expect("Sidebar should exist");

        // Verify toggle button is visible on small screen
        let toggle_visible: bool = ctx
            .eval("getComputedStyle(document.getElementById('sidebar-toggle')).display !== 'none'")
            .await;
        assert!(
            toggle_visible,
            "Toggle button should be visible on small screen"
        );

        // Sidebar should not be collapsed initially
        let initially_collapsed: bool = ctx
            .eval("document.getElementById('sidebar').hasAttribute('data-collapsed')")
            .await;
        assert!(
            !initially_collapsed,
            "Sidebar should not be collapsed initially"
        );

        // Verify aria-expanded is initially true
        let initial_aria: String = ctx
            .eval("document.getElementById('sidebar-toggle').getAttribute('aria-expanded')")
            .await;
        assert_eq!(
            initial_aria, "true",
            "aria-expanded should be true initially"
        );

        // Click the toggle button to collapse
        ctx.page
            .evaluate("document.getElementById('sidebar-toggle').click()")
            .await
            .expect("Failed to click toggle");

        // Verify sidebar is now collapsed
        let is_collapsed: bool = ctx
            .eval("document.getElementById('sidebar').hasAttribute('data-collapsed')")
            .await;
        assert!(
            is_collapsed,
            "Sidebar should be collapsed after clicking toggle"
        );

        // Verify aria-expanded is now false
        let collapsed_aria: String = ctx
            .eval("document.getElementById('sidebar-toggle').getAttribute('aria-expanded')")
            .await;
        assert_eq!(
            collapsed_aria, "false",
            "aria-expanded should be false when collapsed"
        );

        // Click again to expand
        ctx.page
            .evaluate("document.getElementById('sidebar-toggle').click()")
            .await
            .expect("Failed to click toggle");

        // Verify sidebar is expanded again
        let is_expanded: bool = ctx
            .eval("!document.getElementById('sidebar').hasAttribute('data-collapsed')")
            .await;
        assert!(is_expanded, "Sidebar should be expanded after second click");

        // Verify aria-expanded is true again
        let expanded_aria: String = ctx
            .eval("document.getElementById('sidebar-toggle').getAttribute('aria-expanded')")
            .await;
        assert_eq!(
            expanded_aria, "true",
            "aria-expanded should be true when expanded"
        );

        ctx.teardown().await;
    });
}

#[test]
fn test_sidebar_toggle_hidden_on_large_screen() {
    runtime().block_on(async {
        let ctx = setup().await;
        let jwt = JwtConfig::new(b"test-jwt-secret-for-testing");
        let app_base = app_url(&ctx);

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

        // Set viewport to desktop size
        ctx.page
            .execute(
                SetDeviceMetricsOverrideParams::builder()
                    .width(1200)
                    .height(800)
                    .device_scale_factor(1.0)
                    .mobile(false)
                    .build()
                    .unwrap(),
            )
            .await
            .expect("Failed to set viewport");

        // Navigate to app
        ctx.page
            .goto(&format!("{}/index.html", app_base))
            .await
            .expect("Failed to navigate");

        // Wait for app to load
        ctx.wait_for("document.getElementById('sidebar') !== null", 5000)
            .await
            .expect("Sidebar should exist");

        // Verify toggle button is hidden on large screen
        let toggle_hidden: bool = ctx
            .eval("getComputedStyle(document.getElementById('sidebar-toggle')).display === 'none'")
            .await;
        assert!(
            toggle_hidden,
            "Toggle button should be hidden on large screen"
        );

        ctx.teardown().await;
    });
}
