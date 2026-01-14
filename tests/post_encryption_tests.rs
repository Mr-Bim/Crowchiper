//! Browser tests for post encryption.
//! Tests that post content is properly encrypted in the database.

mod common;

use chromiumoxide::cdp::browser_protocol::page::AddScriptToEvaluateOnNewDocumentParams;
use common::{runtime, setup};
use crowchiper::db::UserRole;
use crowchiper::jwt::JwtConfig;

/// Get the app base URL (protected assets)
fn app_url(ctx: &common::TestContext) -> String {
    ctx.base_url.replace("/login", env!("CONFIG_APP_ASSETS"))
}

/// Helper to create an authenticated user with encryption enabled
/// Returns (user_id, test_key) - caller must inject test_key after navigating to app page
async fn setup_authenticated_user_with_encryption(ctx: &common::TestContext) -> (i64, String) {
    let jwt = JwtConfig::new(b"test-jwt-secret-for-testing");

    // Clear any cookies from previous tests
    ctx.clear_cookies().await;

    // Create and activate user
    let user_id = ctx
        .db
        .users()
        .create("test-uuid", "testuser")
        .await
        .unwrap();
    ctx.db.users().activate(user_id).await.unwrap();

    // Enable test encryption in database (no PRF, uses injected key)
    // Note: This generates a test key but we need to inject it after navigation
    let test_key = ctx.enable_test_encryption(user_id).await;

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

    (user_id, test_key)
}

/// Navigate to app and inject test key before page loads
async fn navigate_to_app_with_key(ctx: &common::TestContext, app_base: &str, test_key: &str) {
    // Use CDP to inject the test key before any page scripts run
    // This ensures the key is available when main.ts initializes
    let script = format!("window.__TEST_ENCRYPTION_KEY__ = '{}';", test_key);
    ctx.page
        .execute(AddScriptToEvaluateOnNewDocumentParams::new(script))
        .await
        .expect("Failed to add script to evaluate on new document");

    // Navigate to app
    ctx.page
        .goto(&format!("{}/index.html", app_base))
        .await
        .expect("Failed to navigate");
}

#[test]
fn test_new_post_content_is_encrypted_in_database() {
    runtime().block_on(async {
        let ctx = setup().await;
        let (user_id, test_key) = setup_authenticated_user_with_encryption(&ctx).await;
        let app_base = app_url(&ctx);

        // Navigate to app with test key injection - this should auto-create a post
        navigate_to_app_with_key(&ctx, &app_base, &test_key).await;

        // Wait for editor to be ready
        ctx.wait_for("document.querySelector('.cm-editor') !== null", 5000)
            .await
            .expect("Editor should be visible");

        // Wait for auto-created post
        ctx.wait_for("document.querySelectorAll('.post-item').length >= 1", 5000)
            .await
            .expect("Should have at least 1 post");

        // Type some unique plaintext into the editor
        let unique_plaintext = "UNIQUE_SECRET_PLAINTEXT_12345";
        ctx.page
            .evaluate(format!(
                r#"
                const editor = document.querySelector('.cm-content');
                editor.focus();
                document.execCommand('insertText', false, '{}');
                "#,
                unique_plaintext
            ))
            .await
            .expect("Failed to type in editor");

        // Wait a moment for the encryption debounce (1 second) + server save interval trigger
        // Instead, click the save button to force immediate save
        ctx.wait_for(
            "document.getElementById('save-btn').getAttribute('data-dirty') === 'true'",
            3000,
        )
        .await
        .expect("Save button should show dirty state");

        // Click save button
        ctx.page
            .evaluate("document.getElementById('save-btn').click()")
            .await
            .expect("Failed to click save button");

        // Wait for save to complete (button should show "Saved")
        ctx.wait_for(
            "document.getElementById('save-btn').getAttribute('data-dirty') === 'false'",
            5000,
        )
        .await
        .expect("Save should complete");

        // Check the database - content should be encrypted, not plaintext
        let summaries = ctx.db.posts().list_by_user(user_id).await.unwrap();
        assert_eq!(summaries.len(), 1, "Should have exactly 1 post");

        // Get full post with content
        let post = ctx
            .db
            .posts()
            .get_by_uuid(&summaries[0].uuid, user_id)
            .await
            .unwrap()
            .expect("Post should exist");

        // Verify encryption flags are set
        assert!(
            post.content_encrypted,
            "Post content should be marked as encrypted"
        );
        assert!(
            post.title_encrypted,
            "Post title should be marked as encrypted"
        );

        // Verify the plaintext does NOT appear in the stored content
        assert!(
            !post.content.contains(unique_plaintext),
            "Plaintext '{}' should NOT appear in encrypted content. Got: {}",
            unique_plaintext,
            post.content
        );

        // Verify the plaintext does NOT appear in the title
        let empty_string = String::new();
        let title = post.title.as_ref().unwrap_or(&empty_string);
        assert!(
            !title.contains(unique_plaintext),
            "Plaintext should NOT appear in encrypted title"
        );

        // Verify IV is present (required for decryption)
        assert!(post.iv.is_some(), "Content IV should be present");
        assert!(post.title_iv.is_some(), "Title IV should be present");

        // Verify encryption version is set
        assert!(
            post.encryption_version.is_some(),
            "Encryption version should be set"
        );

        ctx.teardown().await;
    });
}

#[test]
fn test_updated_post_content_is_encrypted_in_database() {
    runtime().block_on(async {
        let ctx = setup().await;
        let (user_id, test_key) = setup_authenticated_user_with_encryption(&ctx).await;
        let app_base = app_url(&ctx);

        // Create a post first (will be encrypted with initial content)
        ctx.db
            .posts()
            .create(
                user_id,
                Some("Initial Title"),
                false,
                None,
                "Initial content",
                false,
                None,
                None,
            )
            .await
            .unwrap();

        // Navigate to app with test key injection
        navigate_to_app_with_key(&ctx, &app_base, &test_key).await;

        // Wait for editor to be ready
        ctx.wait_for("document.querySelector('.cm-editor') !== null", 5000)
            .await
            .expect("Editor should be visible");

        // Wait for post to load
        ctx.wait_for("document.querySelectorAll('.post-item').length >= 1", 5000)
            .await
            .expect("Should have at least 1 post");

        // Select all and type new unique content
        let unique_plaintext = "UPDATED_SECRET_CONTENT_67890";
        ctx.page
            .evaluate(format!(
                r#"
                const editor = document.querySelector('.cm-content');
                editor.focus();
                // Select all
                document.execCommand('selectAll', false, null);
                // Replace with new content
                document.execCommand('insertText', false, '{}');
                "#,
                unique_plaintext
            ))
            .await
            .expect("Failed to type in editor");

        // Wait for dirty state
        ctx.wait_for(
            "document.getElementById('save-btn').getAttribute('data-dirty') === 'true'",
            3000,
        )
        .await
        .expect("Save button should show dirty state");

        // Click save button
        ctx.page
            .evaluate("document.getElementById('save-btn').click()")
            .await
            .expect("Failed to click save button");

        // Wait for save to complete
        ctx.wait_for(
            "document.getElementById('save-btn').getAttribute('data-dirty') === 'false'",
            5000,
        )
        .await
        .expect("Save should complete");

        // Check the database
        let summaries = ctx.db.posts().list_by_user(user_id).await.unwrap();
        assert_eq!(summaries.len(), 1, "Should have exactly 1 post");

        // Get full post with content
        let post = ctx
            .db
            .posts()
            .get_by_uuid(&summaries[0].uuid, user_id)
            .await
            .unwrap()
            .expect("Post should exist");

        // Verify content is encrypted
        assert!(
            post.content_encrypted,
            "Post content should be marked as encrypted"
        );

        // Verify the plaintext does NOT appear in the stored content
        assert!(
            !post.content.contains(unique_plaintext),
            "Plaintext '{}' should NOT appear in encrypted content. Got: {}",
            unique_plaintext,
            post.content
        );

        // Also verify old plaintext is gone
        assert!(
            !post.content.contains("Initial content"),
            "Old plaintext should NOT appear in encrypted content"
        );

        ctx.teardown().await;
    });
}

#[test]
fn test_post_title_extracted_and_encrypted() {
    runtime().block_on(async {
        let ctx = setup().await;
        let (user_id, test_key) = setup_authenticated_user_with_encryption(&ctx).await;
        let app_base = app_url(&ctx);

        // Navigate to app with test key injection - auto-creates a post
        navigate_to_app_with_key(&ctx, &app_base, &test_key).await;

        // Wait for editor
        ctx.wait_for("document.querySelector('.cm-editor') !== null", 5000)
            .await
            .expect("Editor should be visible");

        // Wait for auto-created post
        ctx.wait_for("document.querySelectorAll('.post-item').length >= 1", 5000)
            .await
            .expect("Should have at least 1 post");

        // Type a markdown title
        let secret_title = "SECRET_TITLE_ABCDEF";
        ctx.page
            .evaluate(format!(
                r#"
                const editor = document.querySelector('.cm-content');
                editor.focus();
                document.execCommand('insertText', false, '# {}\n\nSome body content');
                "#,
                secret_title
            ))
            .await
            .expect("Failed to type in editor");

        // Wait for dirty state and save
        ctx.wait_for(
            "document.getElementById('save-btn').getAttribute('data-dirty') === 'true'",
            3000,
        )
        .await
        .expect("Save button should show dirty state");

        ctx.page
            .evaluate("document.getElementById('save-btn').click()")
            .await
            .expect("Failed to click save button");

        ctx.wait_for(
            "document.getElementById('save-btn').getAttribute('data-dirty') === 'false'",
            5000,
        )
        .await
        .expect("Save should complete");

        // Check the database
        let summaries = ctx.db.posts().list_by_user(user_id).await.unwrap();

        // Get full post with content
        let post = ctx
            .db
            .posts()
            .get_by_uuid(&summaries[0].uuid, user_id)
            .await
            .unwrap()
            .expect("Post should exist");

        // Title should be encrypted
        assert!(post.title_encrypted, "Title should be marked as encrypted");
        assert!(post.title_iv.is_some(), "Title IV should be present");

        // The plaintext title should NOT appear in the stored title
        let stored_title = post.title.as_ref().unwrap();
        assert!(
            !stored_title.contains(secret_title),
            "Plaintext title '{}' should NOT appear in encrypted title. Got: {}",
            secret_title,
            stored_title
        );

        ctx.teardown().await;
    });
}

#[test]
fn test_encrypted_content_decrypts_correctly_on_reload() {
    runtime().block_on(async {
        let ctx = setup().await;
        let (user_id, test_key) = setup_authenticated_user_with_encryption(&ctx).await;
        let app_base = app_url(&ctx);

        // Navigate to app with test key injection
        navigate_to_app_with_key(&ctx, &app_base, &test_key).await;

        // Wait for editor
        ctx.wait_for("document.querySelector('.cm-editor') !== null", 5000)
            .await
            .expect("Editor should be visible");

        // Wait for auto-created post
        ctx.wait_for("document.querySelectorAll('.post-item').length >= 1", 5000)
            .await
            .expect("Should have at least 1 post");

        // Type unique content
        let unique_content = "ROUNDTRIP_TEST_CONTENT_XYZ123";
        ctx.page
            .evaluate(format!(
                r#"
                const editor = document.querySelector('.cm-content');
                editor.focus();
                document.execCommand('insertText', false, '{}');
                "#,
                unique_content
            ))
            .await
            .expect("Failed to type in editor");

        // Save
        ctx.wait_for(
            "document.getElementById('save-btn').getAttribute('data-dirty') === 'true'",
            3000,
        )
        .await
        .expect("Save button should show dirty state");

        ctx.page
            .evaluate("document.getElementById('save-btn').click()")
            .await
            .expect("Failed to click save button");

        ctx.wait_for(
            "document.getElementById('save-btn').getAttribute('data-dirty') === 'false'",
            5000,
        )
        .await
        .expect("Save should complete");

        // Verify it's encrypted in DB
        let summaries = ctx.db.posts().list_by_user(user_id).await.unwrap();
        let post = ctx
            .db
            .posts()
            .get_by_uuid(&summaries[0].uuid, user_id)
            .await
            .unwrap()
            .expect("Post should exist");
        assert!(post.content_encrypted, "Content should be encrypted");
        assert!(
            !post.content.contains(unique_content),
            "Plaintext should not be in DB"
        );

        // Reload the page with test key injection
        navigate_to_app_with_key(&ctx, &app_base, &test_key).await;

        // Wait for editor
        ctx.wait_for("document.querySelector('.cm-editor') !== null", 5000)
            .await
            .expect("Editor should be visible");

        // Wait for content to load
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;

        // Verify the content is decrypted and visible
        let editor_content: String = ctx
            .eval("document.querySelector('.cm-content').textContent")
            .await;

        assert!(
            editor_content.contains(unique_content),
            "Decrypted content should contain '{}'. Got: {}",
            unique_content,
            editor_content
        );

        ctx.teardown().await;
    });
}

#[test]
fn test_save_button_shows_unsaved_changes() {
    runtime().block_on(async {
        let ctx = setup().await;
        let (_user_id, test_key) = setup_authenticated_user_with_encryption(&ctx).await;
        let app_base = app_url(&ctx);

        // Navigate to app with test key injection
        navigate_to_app_with_key(&ctx, &app_base, &test_key).await;

        // Wait for editor
        ctx.wait_for("document.querySelector('.cm-editor') !== null", 5000)
            .await
            .expect("Editor should be visible");

        // Initially, save button should show "Saved" (no unsaved changes)
        ctx.wait_for("document.querySelectorAll('.post-item').length >= 1", 5000)
            .await
            .expect("Should have at least 1 post");

        // Check initial state
        let initial_text: String = ctx
            .eval("document.getElementById('save-btn').textContent.trim()")
            .await;
        assert_eq!(initial_text, "Saved", "Initial state should be 'Saved'");

        let initial_dirty: String = ctx
            .eval("document.getElementById('save-btn').getAttribute('data-dirty')")
            .await;
        assert_eq!(
            initial_dirty, "false",
            "Initial data-dirty should be 'false'"
        );

        // Type something
        ctx.page
            .evaluate(
                r#"
                const editor = document.querySelector('.cm-content');
                editor.focus();
                document.execCommand('insertText', false, 'new content');
                "#,
            )
            .await
            .expect("Failed to type in editor");

        // Wait for dirty state
        ctx.wait_for(
            "document.getElementById('save-btn').getAttribute('data-dirty') === 'true'",
            3000,
        )
        .await
        .expect("Save button should show dirty state");

        let dirty_text: String = ctx
            .eval("document.getElementById('save-btn').textContent.trim()")
            .await;
        assert_eq!(dirty_text, "Save", "Dirty state should show 'Save'");

        // Click save
        ctx.page
            .evaluate("document.getElementById('save-btn').click()")
            .await
            .expect("Failed to click save button");

        // Wait for saved state
        ctx.wait_for(
            "document.getElementById('save-btn').getAttribute('data-dirty') === 'false'",
            5000,
        )
        .await
        .expect("Save should complete");

        let final_text: String = ctx
            .eval("document.getElementById('save-btn').textContent.trim()")
            .await;
        assert_eq!(final_text, "Saved", "After save should show 'Saved'");

        ctx.teardown().await;
    });
}
