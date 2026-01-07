mod common;

use common::setup;

#[test]
fn test_theme_toggle_changes_theme() {
    common::runtime().block_on(async {
        let ctx = setup().await;
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
