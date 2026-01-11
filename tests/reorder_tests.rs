//! Browser tests for post drag-and-drop reordering.

mod common;

use common::{runtime, setup};
use crowchiper::db::UserRole;
use crowchiper::jwt::JwtConfig;

/// Get the app base URL (protected assets)
fn app_url(ctx: &common::TestContext) -> String {
    ctx.base_url.replace("/login", env!("CONFIG_APP_ASSETS"))
}

/// Helper to create an authenticated user and set up the app
async fn setup_authenticated_app(ctx: &common::TestContext) -> i64 {
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

    // Mark encryption setup as done (without encryption)
    ctx.db
        .encryption_settings()
        .mark_setup_done(user_id)
        .await
        .unwrap();

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

    user_id
}

#[test]
fn test_posts_display_in_position_order() {
    runtime().block_on(async {
        let ctx = setup().await;
        let user_id = setup_authenticated_app(&ctx).await;
        let app_base = app_url(&ctx);

        // Create 3 posts - they get positions 0, 0, 0 (each shifts others down)
        // So final order is: post3 (0), post2 (1), post1 (2)
        let _post1 = ctx
            .db
            .posts()
            .create(
                user_id,
                Some("First Post"),
                false,
                None,
                "Content 1",
                false,
                None,
                None,
            )
            .await
            .unwrap();
        let _post2 = ctx
            .db
            .posts()
            .create(
                user_id,
                Some("Second Post"),
                false,
                None,
                "Content 2",
                false,
                None,
                None,
            )
            .await
            .unwrap();
        let _post3 = ctx
            .db
            .posts()
            .create(
                user_id,
                Some("Third Post"),
                false,
                None,
                "Content 3",
                false,
                None,
                None,
            )
            .await
            .unwrap();

        // Navigate to app
        ctx.page
            .goto(&format!("{}/index.html", app_base))
            .await
            .expect("Failed to navigate");

        // Wait for posts to load
        ctx.wait_for("document.querySelectorAll('.post-item').length === 3", 5000)
            .await
            .expect("Should have 3 posts");

        // Verify order: Third Post should be first (position 0)
        let first_post_title: String = ctx
            .eval("document.querySelectorAll('.post-item')[0].textContent")
            .await;
        assert_eq!(
            first_post_title, "Third Post",
            "Third post should be first (position 0)"
        );

        let second_post_title: String = ctx
            .eval("document.querySelectorAll('.post-item')[1].textContent")
            .await;
        assert_eq!(
            second_post_title, "Second Post",
            "Second post should be second (position 1)"
        );

        let third_post_title: String = ctx
            .eval("document.querySelectorAll('.post-item')[2].textContent")
            .await;
        assert_eq!(
            third_post_title, "First Post",
            "First post should be third (position 2)"
        );

        ctx.teardown().await;
    });
}

#[test]
fn test_post_items_have_drag_attributes() {
    runtime().block_on(async {
        let ctx = setup().await;
        let user_id = setup_authenticated_app(&ctx).await;
        let app_base = app_url(&ctx);

        // Create a post
        ctx.db
            .posts()
            .create(
                user_id,
                Some("Test Post"),
                false,
                None,
                "Content",
                false,
                None,
                None,
            )
            .await
            .unwrap();

        // Navigate to app
        ctx.page
            .goto(&format!("{}/index.html", app_base))
            .await
            .expect("Failed to navigate");

        // Wait for posts to load
        ctx.wait_for(
            "document.querySelectorAll('.post-wrapper').length === 1",
            5000,
        )
        .await
        .expect("Should have 1 post");

        // Verify post wrapper has data-uuid and data-index attributes for drag and drop
        let has_uuid: bool = ctx
            .eval("document.querySelector('.post-wrapper').hasAttribute('data-uuid')")
            .await;
        assert!(has_uuid, "Post wrapper should have data-uuid attribute");

        let has_index: bool = ctx
            .eval("document.querySelector('.post-wrapper').hasAttribute('data-index')")
            .await;
        assert!(has_index, "Post wrapper should have data-index attribute");

        let index: String = ctx
            .eval("document.querySelector('.post-wrapper').getAttribute('data-index')")
            .await;
        assert_eq!(index, "0", "First post should have index 0");

        ctx.teardown().await;
    });
}

#[test]
fn test_reorder_via_api_updates_display() {
    runtime().block_on(async {
        let ctx = setup().await;
        let user_id = setup_authenticated_app(&ctx).await;
        let app_base = app_url(&ctx);

        // Create 3 posts
        let post1 = ctx
            .db
            .posts()
            .create(
                user_id,
                Some("Post A"),
                false,
                None,
                "Content 1",
                false,
                None,
                None,
            )
            .await
            .unwrap();
        let post2 = ctx
            .db
            .posts()
            .create(
                user_id,
                Some("Post B"),
                false,
                None,
                "Content 2",
                false,
                None,
                None,
            )
            .await
            .unwrap();
        let post3 = ctx
            .db
            .posts()
            .create(
                user_id,
                Some("Post C"),
                false,
                None,
                "Content 3",
                false,
                None,
                None,
            )
            .await
            .unwrap();

        // Current order: C (0), B (1), A (2)
        // Reorder via DB to: A (0), C (1), B (2)
        ctx.db
            .posts()
            .reorder(user_id, &[post1.clone(), post3.clone(), post2.clone()])
            .await
            .unwrap();

        // Navigate to app
        ctx.page
            .goto(&format!("{}/index.html", app_base))
            .await
            .expect("Failed to navigate");

        // Wait for posts to load
        ctx.wait_for("document.querySelectorAll('.post-item').length === 3", 5000)
            .await
            .expect("Should have 3 posts");

        // Verify new order
        let first_title: String = ctx
            .eval("document.querySelectorAll('.post-item')[0].textContent")
            .await;
        assert_eq!(
            first_title, "Post A",
            "Post A should be first after reorder"
        );

        let second_title: String = ctx
            .eval("document.querySelectorAll('.post-item')[1].textContent")
            .await;
        assert_eq!(
            second_title, "Post C",
            "Post C should be second after reorder"
        );

        let third_title: String = ctx
            .eval("document.querySelectorAll('.post-item')[2].textContent")
            .await;
        assert_eq!(
            third_title, "Post B",
            "Post B should be third after reorder"
        );

        ctx.teardown().await;
    });
}

#[test]
fn test_new_post_appears_at_top() {
    runtime().block_on(async {
        let ctx = setup().await;
        let user_id = setup_authenticated_app(&ctx).await;
        let app_base = app_url(&ctx);

        // Create an existing post
        ctx.db
            .posts()
            .create(
                user_id,
                Some("Old Post"),
                false,
                None,
                "Content",
                false,
                None,
                None,
            )
            .await
            .unwrap();

        // Navigate to app
        ctx.page
            .goto(&format!("{}/index.html", app_base))
            .await
            .expect("Failed to navigate");

        // Wait for initial post to load
        ctx.wait_for("document.querySelectorAll('.post-item').length === 1", 5000)
            .await
            .expect("Should have 1 post");

        // Click new post button
        ctx.page
            .evaluate("document.getElementById('new-post-btn').click()")
            .await
            .expect("Failed to click new post button");

        // Wait for new post to appear
        ctx.wait_for("document.querySelectorAll('.post-item').length === 2", 5000)
            .await
            .expect("Should have 2 posts");

        // Verify the new post (Untitled) is at the top
        let first_title: String = ctx
            .eval("document.querySelectorAll('.post-item')[0].textContent")
            .await;
        assert_eq!(first_title, "Untitled", "New post should be at the top");

        let second_title: String = ctx
            .eval("document.querySelectorAll('.post-item')[1].textContent")
            .await;
        assert_eq!(second_title, "Old Post", "Old post should be second");

        ctx.teardown().await;
    });
}

#[test]
fn test_post_items_have_grab_cursor() {
    runtime().block_on(async {
        let ctx = setup().await;
        let user_id = setup_authenticated_app(&ctx).await;
        let app_base = app_url(&ctx);

        // Create a post
        ctx.db
            .posts()
            .create(
                user_id,
                Some("Test Post"),
                false,
                None,
                "Content",
                false,
                None,
                None,
            )
            .await
            .unwrap();

        // Navigate to app
        ctx.page
            .goto(&format!("{}/index.html", app_base))
            .await
            .expect("Failed to navigate");

        // Wait for posts to load
        ctx.wait_for(
            "document.querySelectorAll('.post-wrapper').length === 1",
            5000,
        )
        .await
        .expect("Should have 1 post");

        // Verify the cursor style indicates draggability on the button (drag handle)
        let cursor: String = ctx
            .eval("getComputedStyle(document.querySelector('.post-item')).cursor")
            .await;
        assert_eq!(cursor, "grab", "Post item should have grab cursor");

        ctx.teardown().await;
    });
}
