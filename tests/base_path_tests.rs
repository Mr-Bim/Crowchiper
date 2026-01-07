mod common;

use common::setup_with_base;

#[test]
fn test_login_page_with_base_path() {
    common::runtime().block_on(async {
        let ctx = setup_with_base(Some("/app")).await;
        ctx.goto("/index.html").await;

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
fn test_register_page_with_base_path() {
    common::runtime().block_on(async {
        let ctx = setup_with_base(Some("/myapp")).await;
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
fn test_assets_load_with_base_path() {
    common::runtime().block_on(async {
        let ctx = setup_with_base(Some("/crowchiper")).await;
        ctx.goto("/index.html").await;

        ctx.wait_for("!document.getElementById('login-button')?.disabled", 5000)
            .await
            .expect("Login button should be enabled - JS assets must have loaded correctly with base path");

        ctx.teardown().await;
    });
}

#[test]
fn test_navigation_with_base_path() {
    common::runtime().block_on(async {
        let ctx = setup_with_base(Some("/test")).await;
        ctx.goto("/index.html").await;

        // Click the register link
        ctx.page
            .evaluate("document.querySelector('a[href*=\"register\"]').click()")
            .await
            .expect("Failed to click");

        ctx.wait_for("document.title === 'Register - Crowchiper'", 5000)
            .await
            .expect("Should navigate to register page");

        // Verify URL contains the base path
        let url: String = ctx.eval("window.location.href").await;
        let expected_path = format!("/test{}/register.html", env!("CONFIG_LOGIN_ASSETS"));
        assert!(
            url.contains(&expected_path),
            "URL should contain base path, expected '{}' in '{}'",
            expected_path,
            url
        );

        ctx.teardown().await;
    });
}

#[test]
fn test_theme_with_base_path() {
    common::runtime().block_on(async {
        let ctx = setup_with_base(Some("/custom")).await;
        ctx.goto("/index.html").await;

        // Wait for theme toggle to be created
        ctx.wait_for("document.getElementById('theme-select') !== null", 5000)
            .await
            .expect("Theme select should exist");

        let initial_theme: String = ctx
            .eval("document.documentElement.getAttribute('data-theme') || ''")
            .await;

        // Select a different theme
        let new_theme = if initial_theme == "warm-light" {
            "scandi-dark"
        } else {
            "warm-light"
        };
        let js = format!(
            "document.getElementById('theme-select').value = '{}'; document.getElementById('theme-select').dispatchEvent(new Event('change'))",
            new_theme
        );
        ctx.page.evaluate(js).await.expect("Failed to change theme");

        // Verify theme changed
        let current_theme: String = ctx
            .eval("document.documentElement.getAttribute('data-theme') || ''")
            .await;
        assert_eq!(current_theme, new_theme, "Theme should have changed");

        ctx.teardown().await;
    });
}
