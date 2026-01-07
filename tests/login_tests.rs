mod common;

use common::TestSetup;
use std::time::Duration;

#[test]
fn test_page_loads() {
    common::runtime().block_on(async {
        let ctx = TestSetup::new().with_clear_cookies().build().await;
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
fn test_button_enabled_after_js_loads() {
    common::runtime().block_on(async {
        let ctx = TestSetup::new().with_clear_cookies().build().await;
        ctx.goto("/index.html").await;

        ctx.wait_for("!document.getElementById('login-button')?.disabled", 5000)
            .await
            .expect("Login button should be enabled after JS loads");

        ctx.teardown().await;
    });
}

#[test]
fn test_username_input() {
    common::runtime().block_on(async {
        let ctx = TestSetup::new().with_clear_cookies().build().await;
        ctx.goto("/index.html").await;

        // Wait for the page to fully load and JS to initialize (login button enabled means JS ran)
        ctx.wait_for("!document.getElementById('login-button')?.disabled", 5000)
            .await
            .expect("Login button should be enabled after JS loads");

        let is_visible: bool = ctx
            .eval("document.getElementById('username')?.offsetParent !== null")
            .await;
        assert!(is_visible, "Username input should be visible");

        // Type in the input
        ctx.page
            .evaluate("document.getElementById('username').value = 'testuser'")
            .await
            .expect("Failed to set value");

        let value: String = ctx.eval("document.getElementById('username').value").await;
        assert_eq!(value, "testuser");

        ctx.teardown().await;
    });
}

#[test]
fn test_register_link_navigates() {
    common::runtime().block_on(async {
        let ctx = TestSetup::new().with_clear_cookies().build().await;
        ctx.goto("/index.html").await;

        // Wait for the register link to become visible (after config fetch completes)
        ctx.wait_for(
            "document.getElementById('register-link')?.hasAttribute('data-visible')",
            5000,
        )
        .await
        .expect("Register link should be visible");

        ctx.page
            .evaluate("document.getElementById('register-link').click()")
            .await
            .expect("Failed to click");

        ctx.wait_for("document.title === 'Register - Crowchiper'", 5000)
            .await
            .expect("Should navigate to register page");

        ctx.teardown().await;
    });
}

#[test]
fn test_login_success() {
    common::runtime().block_on(async {
        let ctx = TestSetup::new().with_clear_cookies().build().await;

        // First, register a user via the register page
        ctx.goto("/register.html").await;
        ctx.wait_for(
            "!document.getElementById('register-button')?.disabled",
            5000,
        )
        .await
        .expect("Register button should be enabled");

        ctx.page
            .find_element("#username")
            .await
            .expect("Failed to find username input")
            .click()
            .await
            .expect("Failed to click input")
            .type_str("loginuser")
            .await
            .expect("Failed to type username");

        ctx.page
            .find_element("#register-button")
            .await
            .expect("Failed to find register button")
            .click()
            .await
            .expect("Failed to click register");

        // Wait for user to be activated
        let mut user_activated = false;
        for _ in 0..50 {
            if let Some(user) = ctx.db.users().get_by_username("loginuser").await.unwrap() {
                if user.activated {
                    user_activated = true;
                    break;
                }
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        assert!(
            user_activated,
            "User should be activated after registration"
        );

        // Registration now redirects directly to encryption setup (user is logged in)
        ctx.wait_for(
            &format!(
                "window.location.pathname.includes('{}/setup-encryption.html')",
                env!("CONFIG_APP_ASSETS")
            ),
            5000,
        )
        .await
        .expect("Should redirect to encryption setup after registration");

        // Clear cookies to test the login flow separately
        ctx.clear_cookies().await;

        // Navigate to login page
        ctx.goto("/index.html").await;

        // Wait for login button to be enabled
        ctx.wait_for("!document.getElementById('login-button')?.disabled", 5000)
            .await
            .expect("Login button should be enabled");

        // Set username via JavaScript (more reliable than find_element after navigation)
        ctx.page
            .evaluate("document.getElementById('username').value = 'loginuser'")
            .await
            .expect("Failed to set username");

        // Click login button via JavaScript
        ctx.page
            .evaluate("document.getElementById('login-button').click()")
            .await
            .expect("Failed to click login");

        // Wait for redirect to encryption setup (user hasn't set up encryption yet)
        ctx.wait_for(
            &format!(
                "window.location.pathname.includes('{}/setup-encryption.html')",
                env!("CONFIG_APP_ASSETS")
            ),
            5000,
        )
        .await
        .expect("Should redirect to encryption setup after login");

        ctx.teardown().await;
    });
}

#[test]
fn test_login_redirects_to_app_with_base_path() {
    common::runtime().block_on(async {
        let ctx = TestSetup::new()
            .with_base(Some("/myapp"))
            .with_clear_cookies()
            .build()
            .await;

        // First, register a user
        ctx.goto("/register.html").await;
        ctx.wait_for(
            "!document.getElementById('register-button')?.disabled",
            5000,
        )
        .await
        .expect("Register button should be enabled");

        ctx.page
            .find_element("#username")
            .await
            .expect("Failed to find username input")
            .click()
            .await
            .expect("Failed to click input")
            .type_str("baseuser")
            .await
            .expect("Failed to type username");

        ctx.page
            .find_element("#register-button")
            .await
            .expect("Failed to find register button")
            .click()
            .await
            .expect("Failed to click register");

        // Wait for user to be activated
        let mut user_activated = false;
        for _ in 0..50 {
            if let Some(user) = ctx.db.users().get_by_username("baseuser").await.unwrap() {
                if user.activated {
                    user_activated = true;
                    break;
                }
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        assert!(
            user_activated,
            "User should be activated after registration"
        );

        // Registration now redirects directly to encryption setup with base path (user is logged in)
        ctx.wait_for(
            &format!(
                "window.location.pathname.includes('/myapp{}/setup-encryption.html')",
                env!("CONFIG_APP_ASSETS")
            ),
            5000,
        )
        .await
        .expect("Should redirect to encryption setup after registration");

        // Verify URL contains both base path and app path
        let url: String = ctx.eval("window.location.href").await;
        assert!(
            url.contains("/myapp")
                && url.contains(env!("CONFIG_APP_ASSETS"))
                && url.contains("setup-encryption.html"),
            "Should be on encryption setup page with base path, got: {}",
            url
        );

        // Clear cookies to test the login flow separately
        ctx.clear_cookies().await;

        // Navigate to login page
        ctx.goto("/index.html").await;

        // Now login with the registered user
        ctx.wait_for("!document.getElementById('login-button')?.disabled", 5000)
            .await
            .expect("Login button should be enabled");

        // Set username via JavaScript (more reliable than find_element after navigation)
        ctx.page
            .evaluate("document.getElementById('username').value = 'baseuser'")
            .await
            .expect("Failed to set username");

        // Click login button via JavaScript
        ctx.page
            .evaluate("document.getElementById('login-button').click()")
            .await
            .expect("Failed to click login");

        // Wait for redirect to encryption setup with base path (user hasn't set up encryption yet)
        ctx.wait_for(
            &format!(
                "window.location.pathname.includes('/myapp{}/setup-encryption.html')",
                env!("CONFIG_APP_ASSETS")
            ),
            5000,
        )
        .await
        .expect("Should redirect to encryption setup with base path after login");

        // Verify URL contains both base path and app path
        let url: String = ctx.eval("window.location.href").await;
        assert!(
            url.contains("/myapp")
                && url.contains(env!("CONFIG_APP_ASSETS"))
                && url.contains("setup-encryption.html"),
            "Should be on encryption setup page with base path, got: {}",
            url
        );

        ctx.teardown().await;
    });
}

#[test]
fn test_register_link_hidden_when_signup_disabled() {
    common::runtime().block_on(async {
        let ctx = TestSetup::new()
            .with_no_signup(true)
            .with_clear_cookies()
            .build()
            .await;

        ctx.goto("/index.html").await;

        // Wait for JS to load
        ctx.wait_for("!document.getElementById('login-button')?.disabled", 5000)
            .await
            .expect("Login button should be enabled");

        // Wait a bit for config to be fetched and processed
        tokio::time::sleep(Duration::from_millis(200)).await;

        // Check that register link is hidden
        let register_link_visible: bool = ctx
            .page
            .evaluate(
                "(() => {
                    const link = document.getElementById('register-link');
                    if (!link) return false;
                    const style = window.getComputedStyle(link);
                    return style.display !== 'none';
                })()",
            )
            .await
            .expect("Failed to check register link visibility")
            .into_value()
            .expect("Failed to deserialize");
        assert!(
            !register_link_visible,
            "Register link should be hidden when signups are disabled"
        );

        ctx.teardown().await;
    });
}

#[test]
fn test_register_link_visible_when_signup_enabled() {
    common::runtime().block_on(async {
        let ctx = TestSetup::new().with_clear_cookies().build().await;
        ctx.goto("/index.html").await;

        // Wait for JS to load
        ctx.wait_for("!document.getElementById('login-button')?.disabled", 5000)
            .await
            .expect("Login button should be enabled");

        // Wait for register link to become visible (display changes from none to block)
        ctx.wait_for(
            "window.getComputedStyle(document.getElementById('register-link')).display === 'block'",
            5000,
        )
        .await
        .expect("Register link should become visible when signups are enabled");

        // Verify it has the data-visible attribute
        let has_visible_attr: bool = ctx
            .eval("document.getElementById('register-link').hasAttribute('data-visible')")
            .await;
        assert!(
            has_visible_attr,
            "Register link should have 'data-visible' attribute"
        );

        ctx.teardown().await;
    });
}
