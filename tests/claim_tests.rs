mod common;

use common::setup;
use std::time::Duration;

#[test]
fn test_claim_page_loads() {
    common::runtime().block_on(async {
        let ctx = setup().await;
        ctx.goto("/claim.html").await;

        let title = ctx
            .page
            .get_title()
            .await
            .expect("Failed to get title")
            .unwrap_or_default();
        assert_eq!(title, "Claim Account - Crowchiper");

        ctx.teardown().await;
    });
}

#[test]
fn test_claim_page_without_uuid_shows_error() {
    common::runtime().block_on(async {
        let ctx = setup().await;
        ctx.goto("/claim.html").await;

        // Wait for JS to load
        tokio::time::sleep(Duration::from_millis(200)).await;

        let status: String = ctx
            .page
            .evaluate("document.getElementById('status')?.textContent || ''")
            .await
            .expect("Failed to get status")
            .into_value()
            .expect("Failed to deserialize");

        assert!(
            status.contains("Invalid"),
            "Should show invalid link message, got: {}",
            status
        );

        // Button should remain disabled
        let disabled: bool = ctx
            .page
            .evaluate("document.getElementById('claim-button')?.disabled ?? true")
            .await
            .expect("Failed to check button")
            .into_value()
            .expect("Failed to deserialize");

        assert!(disabled, "Claim button should be disabled without UUID");

        ctx.teardown().await;
    });
}

#[test]
fn test_claim_admin_user_success() {
    common::runtime().block_on(async {
        let ctx = setup().await;

        // Create an admin user directly in the database
        let uuid = uuid::Uuid::new_v4().to_string();
        let username = "TestAdmin";
        ctx.db
            .users()
            .create_admin(&uuid, username)
            .await
            .expect("Failed to create admin user");

        // Verify user exists but is not activated
        let user = ctx
            .db
            .users()
            .get_by_uuid(&uuid)
            .await
            .unwrap()
            .expect("Admin user should exist");
        assert!(!user.activated, "Admin user should not be activated yet");
        assert_eq!(user.username, username);

        // Navigate to claim page with UUID
        ctx.goto(&format!("/claim.html?uuid={}", uuid)).await;

        // Wait for button to be enabled
        ctx.wait_for("!document.getElementById('claim-button')?.disabled", 5000)
            .await
            .expect("Claim button should be enabled");

        // Check status text
        let status: String = ctx
            .page
            .evaluate("document.getElementById('status')?.textContent || ''")
            .await
            .expect("Failed to get status")
            .into_value()
            .expect("Failed to deserialize");
        assert!(
            status.contains("Click") && status.contains("passkey"),
            "Should show instruction to register passkey, got: {}",
            status
        );

        // Click the claim button
        ctx.page
            .find_element("#claim-button")
            .await
            .expect("Failed to find claim button")
            .click()
            .await
            .expect("Failed to click claim button");

        // Wait for user to be activated (virtual authenticator completes registration automatically)
        let mut user_activated = false;
        for _ in 0..50 {
            if let Some(user) = ctx.db.users().get_by_uuid(&uuid).await.unwrap() {
                if user.activated {
                    user_activated = true;
                    break;
                }
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        assert!(user_activated, "Admin user should be activated after claim");

        // Wait for redirect to login page
        ctx.wait_for("document.title === 'Crowchiper'", 5000)
            .await
            .expect("Should redirect to login page after claiming");

        ctx.teardown().await;
    });
}

#[test]
fn test_claim_with_invalid_uuid_fails() {
    common::runtime().block_on(async {
        let ctx = setup().await;

        // Navigate to claim page with a non-existent UUID
        let fake_uuid = uuid::Uuid::new_v4().to_string();
        ctx.goto(&format!("/claim.html?uuid={}", fake_uuid)).await;

        // Wait for button to be enabled
        ctx.wait_for("!document.getElementById('claim-button')?.disabled", 5000)
            .await
            .expect("Claim button should be enabled");

        // Click the claim button
        ctx.page
            .find_element("#claim-button")
            .await
            .expect("Failed to find claim button")
            .click()
            .await
            .expect("Failed to click claim button");

        // Wait for error message to appear in status
        let mut error_shown = false;
        for _ in 0..50 {
            let status: String = ctx
                .page
                .evaluate("document.getElementById('status')?.textContent || ''")
                .await
                .expect("Failed to get status")
                .into_value()
                .expect("Failed to deserialize");

            if status.contains("not found") || status.contains("Failed") {
                error_shown = true;
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        assert!(error_shown, "Should show error for invalid UUID");

        ctx.teardown().await;
    });
}
