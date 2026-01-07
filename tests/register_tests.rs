mod common;

use common::{TestSetup, setup};
use std::time::{Duration, Instant};

#[test]
fn test_authenticator_type_hidden_on_non_android() {
    common::runtime().block_on(async {
        let ctx = setup().await;
        ctx.goto("/register.html").await;

        // Wait for JS to load
        ctx.wait_for(
            "!document.getElementById('register-button')?.disabled",
            5000,
        )
        .await
        .expect("Button should be enabled");

        // Verify fieldset is hidden on non-Android (Chrome headless is not Android)
        let fieldset_hidden: bool = ctx
            .page
            .evaluate("document.getElementById('auth-type-fieldset')?.hidden")
            .await
            .expect("Failed to check fieldset hidden")
            .into_value()
            .expect("Failed to deserialize");
        assert!(
            fieldset_hidden,
            "Authenticator type fieldset should be hidden on non-Android"
        );

        ctx.teardown().await;
    });
}

#[test]
fn test_authenticator_type_shown_on_android() {
    common::runtime().block_on(async {
        let ctx = setup().await;

        // Set Android user agent before navigating
        ctx.page
            .execute(
                chromiumoxide::cdp::browser_protocol::emulation::SetUserAgentOverrideParams::new(
                    "Mozilla/5.0 (Linux; Android 14) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Mobile Safari/537.36".to_string()
                )
            )
            .await
            .expect("Failed to set user agent");

        ctx.goto("/register.html").await;

        // Wait for JS to load
        ctx.wait_for(
            "!document.getElementById('register-button')?.disabled",
            5000,
        )
        .await
        .expect("Button should be enabled");

        // Verify fieldset is visible on Android
        let fieldset_hidden: bool = ctx
            .page
            .evaluate("document.getElementById('auth-type-fieldset')?.hidden")
            .await
            .expect("Failed to check fieldset hidden")
            .into_value()
            .expect("Failed to deserialize");
        assert!(!fieldset_hidden, "Authenticator type fieldset should be visible on Android");

        // Verify passkey is selected by default
        let passkey_checked: bool = ctx
            .page
            .evaluate("document.querySelector('input[name=\"auth-type\"][value=\"passkey\"]').checked")
            .await
            .expect("Failed to check if passkey is checked")
            .into_value()
            .expect("Failed to deserialize");
        assert!(passkey_checked, "Passkey should be selected by default on Android");

        ctx.teardown().await;
    });
}

#[test]
fn test_page_loads() {
    common::runtime().block_on(async {
        let ctx = setup().await;
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
fn test_button_enabled_after_js_loads() {
    common::runtime().block_on(async {
        let ctx = setup().await;
        ctx.goto("/register.html").await;

        ctx.wait_for(
            "!document.getElementById('register-button')?.disabled",
            5000,
        )
        .await
        .expect("Register button should be enabled after JS loads");

        ctx.teardown().await;
    });
}

#[test]
fn test_login_link_navigates() {
    common::runtime().block_on(async {
        let ctx = setup().await;
        ctx.goto("/register.html").await;

        ctx.page
            .evaluate("document.querySelector('a[href*=\"index\"]').click()")
            .await
            .expect("Failed to click");

        ctx.wait_for("document.title === 'Crowchiper'", 5000)
            .await
            .expect("Should navigate to login page");

        ctx.teardown().await;
    });
}

#[test]
fn test_claim_username_success() {
    common::runtime().block_on(async {
        let ctx = setup().await;
        ctx.goto("/register.html").await;

        // Wait for JS to load
        ctx.wait_for(
            "!document.getElementById('register-button')?.disabled",
            5000,
        )
        .await
        .expect("Button should be enabled");

        // Verify no users exist initially
        assert!(
            ctx.db
                .users()
                .get_by_username("testuser")
                .await
                .unwrap()
                .is_none(),
            "User should not exist before registration"
        );

        // Type username and click register
        ctx.page
            .find_element("#username")
            .await
            .expect("Failed to find username input")
            .click()
            .await
            .expect("Failed to click input")
            .type_str("testuser")
            .await
            .expect("Failed to type username");

        // Click register button
        ctx.page
            .find_element("#register-button")
            .await
            .expect("Failed to find register button")
            .click()
            .await
            .expect("Failed to click register");

        // Wait for user to be activated (virtual authenticator completes registration automatically)
        let mut user_activated = false;
        for _ in 0..50 {
            if let Some(user) = ctx.db.users().get_by_username("testuser").await.unwrap() {
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

        // Wait for redirect to setup-encryption page (user is now logged in)
        ctx.wait_for("document.title === 'Setup Encryption - Crowchiper'", 5000)
            .await
            .expect("Should redirect to setup-encryption page after registration");

        ctx.teardown().await;
    });
}

#[test]
fn test_claim_duplicate_username_fails() {
    common::runtime().block_on(async {
        let ctx = TestSetup::new().with_clear_cookies().build().await;
        ctx.goto("/register.html").await;

        // Wait for JS to load
        ctx.wait_for(
            "!document.getElementById('register-button')?.disabled",
            5000,
        )
        .await
        .expect("Button should be enabled");

        // Claim first username
        ctx.page
            .find_element("#username")
            .await
            .expect("Failed to find username input")
            .click()
            .await
            .expect("Failed to click input")
            .type_str("duplicateuser")
            .await
            .expect("Failed to type username");

        ctx.page
            .find_element("#register-button")
            .await
            .expect("Failed to find register button")
            .click()
            .await
            .expect("Failed to click register");

        // Wait for user to appear in database
        let mut user_exists = false;
        for _ in 0..50 {
            if ctx
                .db
                .users()
                .get_by_username("duplicateuser")
                .await
                .unwrap()
                .is_some()
            {
                user_exists = true;
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        assert!(user_exists, "First user should exist");

        // Clear cookies before opening second page (first registration set a cookie)
        ctx.clear_cookies().await;

        // Open a new page and try to claim the same username
        let page2 = ctx.new_page().await;

        page2
            .goto(&format!("{}/register.html", ctx.base_url))
            .await
            .expect("Failed to navigate");

        // Wait for JS to load on page2
        let start = Instant::now();
        while start.elapsed() < Duration::from_secs(5) {
            let disabled: bool = page2
                .evaluate("document.getElementById('register-button')?.disabled ?? true")
                .await
                .expect("Failed to check button")
                .into_value()
                .expect("Failed to deserialize");
            if !disabled {
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }

        // Try to claim the same username
        page2
            .find_element("#username")
            .await
            .expect("Failed to find username input")
            .click()
            .await
            .expect("Failed to click input")
            .type_str("duplicateuser")
            .await
            .expect("Failed to type username");

        page2
            .find_element("#register-button")
            .await
            .expect("Failed to find register button")
            .click()
            .await
            .expect("Failed to click register");

        // Wait for error message to be shown
        let start = Instant::now();
        let mut error_msg = String::new();
        while start.elapsed() < Duration::from_secs(5) {
            error_msg = page2
                .evaluate("document.getElementById('error-message')?.textContent || ''")
                .await
                .expect("Failed to get error message")
                .into_value()
                .expect("Failed to deserialize");
            if !error_msg.is_empty() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }

        assert!(
            error_msg.contains("already taken"),
            "Should show error for duplicate username, got: {}",
            error_msg
        );

        // Verify still only one user in database
        assert!(
            !ctx.db
                .users()
                .is_username_available("duplicateuser")
                .await
                .unwrap(),
            "Username should still be taken"
        );

        ctx.teardown().await;
    });
}

#[test]
fn test_multiple_users_can_register() {
    common::runtime().block_on(async {
        let ctx = setup().await;

        // Register first user
        ctx.goto("/register.html").await;
        ctx.wait_for(
            "!document.getElementById('register-button')?.disabled",
            5000,
        )
        .await
        .expect("Button should be enabled");

        ctx.page
            .find_element("#username")
            .await
            .expect("Failed to find username input")
            .click()
            .await
            .expect("Failed to click input")
            .type_str("firstuser")
            .await
            .expect("Failed to type username");

        ctx.page
            .find_element("#register-button")
            .await
            .expect("Failed to find register button")
            .click()
            .await
            .expect("Failed to click register");

        // Wait for first user to be activated (registration completes with virtual authenticator)
        let mut first_activated = false;
        for _ in 0..50 {
            if let Some(user) = ctx.db.users().get_by_username("firstuser").await.unwrap() {
                if user.activated {
                    first_activated = true;
                    break;
                }
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        assert!(first_activated, "First user should be activated");

        // Wait for redirect to setup-encryption page (user is now logged in)
        ctx.wait_for("document.title === 'Setup Encryption - Crowchiper'", 5000)
            .await
            .expect("Should redirect to setup-encryption page");

        // Register second user in a new page
        let page2 = ctx.new_page().await;
        page2
            .goto(&format!("{}/register.html", ctx.base_url))
            .await
            .expect("Failed to navigate");

        // Wait for JS to load
        let start = Instant::now();
        while start.elapsed() < Duration::from_secs(5) {
            let disabled: bool = page2
                .evaluate("document.getElementById('register-button')?.disabled ?? true")
                .await
                .expect("Failed to check button")
                .into_value()
                .expect("Failed to deserialize");
            if !disabled {
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }

        page2
            .find_element("#username")
            .await
            .expect("Failed to find username input")
            .click()
            .await
            .expect("Failed to click input")
            .type_str("seconduser")
            .await
            .expect("Failed to type username");

        page2
            .find_element("#register-button")
            .await
            .expect("Failed to find register button")
            .click()
            .await
            .expect("Failed to click register");

        // Wait for second user to be activated
        let mut second_activated = false;
        for _ in 0..50 {
            if let Some(user) = ctx.db.users().get_by_username("seconduser").await.unwrap() {
                if user.activated {
                    second_activated = true;
                    break;
                }
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        assert!(second_activated, "Second user should be activated");

        // Verify both users exist and are activated
        let first = ctx
            .db
            .users()
            .get_by_username("firstuser")
            .await
            .unwrap()
            .unwrap();
        let second = ctx
            .db
            .users()
            .get_by_username("seconduser")
            .await
            .unwrap()
            .unwrap();
        assert!(first.activated, "First user should still be activated");
        assert!(second.activated, "Second user should be activated");

        ctx.teardown().await;
    });
}

#[test]
fn test_register_page_redirects_when_signup_disabled() {
    common::runtime().block_on(async {
        let ctx = TestSetup::new()
            .with_no_signup(true)
            .with_clear_cookies()
            .build()
            .await;

        // Navigate to register page
        ctx.goto("/register.html").await;

        // Wait for redirect to login page
        ctx.wait_for("document.title === 'Crowchiper'", 5000)
            .await
            .expect("Should redirect to login page when signups are disabled");

        // Verify we're on the login page by checking for the login button
        let has_login_button: bool = ctx
            .page
            .evaluate("!!document.getElementById('login-button')")
            .await
            .expect("Failed to check for login button")
            .into_value()
            .expect("Failed to deserialize");
        assert!(
            has_login_button,
            "Should be on login page with login button"
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

        // Navigate to login page
        ctx.goto("/index.html").await;

        // Wait for JS to load (passkey button gets enabled)
        ctx.wait_for("!document.getElementById('passkey-button')?.disabled", 5000)
            .await
            .expect("Passkey button should be enabled");

        // Check that register link is hidden (display: none or not visible)
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

        // Navigate to login page
        ctx.goto("/index.html").await;

        // Wait for JS to load (passkey button gets enabled)
        ctx.wait_for("!document.getElementById('passkey-button')?.disabled", 5000)
            .await
            .expect("Passkey button should be enabled");

        // Wait for register link to become visible (config fetched and data-visible attribute added)
        ctx.wait_for(
            "document.getElementById('register-link')?.hasAttribute('data-visible')",
            5000,
        )
        .await
        .expect("Register link should become visible when signups are enabled");

        ctx.teardown().await;
    });
}
