import {
  test,
  expect,
  createUser,
  uniqueTestId,
  CreateUserResult,
} from "../utils/fixtures.ts";
import { getServer, Server } from "../utils/server.ts";
import { BrowserContext } from "@playwright/test";

test.describe("Post navigation", () => {
  let context: BrowserContext;
  let userResult: CreateUserResult;
  let baseUrl: string;

  test.beforeAll(async ({ browser }) => {
    const server = await getServer(Server.Default);
    baseUrl = server.baseUrl;
    context = await browser.newContext();
    const username = `postnav_${uniqueTestId()}`;
    userResult = await createUser({
      context,
      baseUrl,
      username,
      enableEncryption: true,
    });
  });

  test.afterAll(async () => {
    await context?.close();
  });

  test("switching posts preserves content in both posts", async () => {
    const { page } = userResult;

    const postList = page.locator("#post-list");
    const editor = page.locator('[data-testid="test-editor-content"]');

    // Wait for initial post
    await expect(
      postList.locator('[data-testid="test-post-wrapper"]'),
    ).toHaveCount(1, { timeout: 10000 });

    // Type content in first post
    await editor.click();
    await page.keyboard.type("First post content");

    // Wait for title to update
    await expect(
      postList
        .locator('[data-testid="test-post-item"]')
        .filter({ hasText: "First post content" }),
    ).toBeVisible({ timeout: 5000 });

    // Create second post
    await page.locator("#new-post-btn").click();
    await expect(
      postList.locator('[data-testid="test-post-wrapper"]'),
    ).toHaveCount(2, { timeout: 5000 });

    // Type content in second post
    await editor.click();
    await page.keyboard.type("Second post content");

    // Wait for title to update
    await expect(
      postList
        .locator('[data-testid="test-post-item"]')
        .filter({ hasText: "Second post content" }),
    ).toBeVisible({ timeout: 5000 });

    // Switch back to first post
    await postList
      .locator('[data-testid="test-post-item"]')
      .filter({ hasText: "First post content" })
      .click();

    // Verify first post content is loaded
    await expect(editor).toContainText("First post content", { timeout: 5000 });

    // Switch to second post
    await postList
      .locator('[data-testid="test-post-item"]')
      .filter({ hasText: "Second post content" })
      .click();

    // Verify second post content is loaded
    await expect(editor).toContainText("Second post content", {
      timeout: 5000,
    });
  });

  test("rapid post switching does not corrupt content", async () => {
    const { page } = userResult;

    const postList = page.locator("#post-list");
    const editor = page.locator('[data-testid="test-editor-content"]');

    // Get the two posts from previous test
    const firstPost = postList
      .locator('[data-testid="test-post-item"]')
      .filter({ hasText: "First post content" });
    const secondPost = postList
      .locator('[data-testid="test-post-item"]')
      .filter({ hasText: "Second post content" });

    // Rapidly switch between posts multiple times
    for (let i = 0; i < 5; i++) {
      await secondPost.click();
      await page.waitForTimeout(100);
      await firstPost.click();
      await page.waitForTimeout(100);
    }

    // End on second post
    await secondPost.click();
    await page.waitForTimeout(500);

    // Verify content is correct (second post)
    await expect(editor).toContainText("Second post content", {
      timeout: 5000,
    });

    // Switch to first and verify
    await firstPost.click();
    await page.waitForTimeout(500);
    await expect(editor).toContainText("First post content", { timeout: 5000 });
  });

  test("selected post is visually highlighted", async () => {
    const { page } = userResult;

    const postList = page.locator("#post-list");

    // Get post items by their text content (more reliable than index)
    const firstPost = postList
      .locator('[data-testid="test-post-item"]')
      .filter({ hasText: "First post content" });
    const secondPost = postList
      .locator('[data-testid="test-post-item"]')
      .filter({ hasText: "Second post content" });

    // Click on first post
    await firstPost.click();

    // First post should have 'active' class
    await expect(firstPost).toHaveClass(/active/, { timeout: 5000 });

    // Second post should not have 'active' class
    await expect(secondPost).not.toHaveClass(/active/);

    // Click on second post
    await secondPost.click();

    // Now second should be active, first should not
    await expect(secondPost).toHaveClass(/active/, { timeout: 5000 });
    await expect(firstPost).not.toHaveClass(/active/);
  });

  test("content persists after page reload", async () => {
    const { page } = userResult;

    const postList = page.locator("#post-list");
    const editor = page.locator('[data-testid="test-editor-content"]');
    const syncIndicator = page.locator('[data-testid="test-sync-indicator"]');
    const forceSaveBtn = page.locator('[data-testid="test-force-save-btn"]');

    // Get current post count
    const countBefore = await postList
      .locator('[data-testid="test-post-wrapper"]')
      .count();

    // Create a new post with unique content for this test
    await page.locator("#new-post-btn").click();

    // Wait for new post
    await expect(
      postList.locator('[data-testid="test-post-wrapper"]'),
    ).toHaveCount(countBefore + 1, { timeout: 5000 });

    // Type unique content
    await editor.click();
    await page.keyboard.type("Persist test content");

    // Wait for title to appear in sidebar
    await expect(
      postList
        .locator('[data-testid="test-post-item"]')
        .filter({ hasText: "Persist test content" }),
    ).toBeVisible({ timeout: 5000 });

    // Force save and wait for sync to complete
    await forceSaveBtn.click();
    await expect(syncIndicator).toHaveAttribute("data-status", "synced", {
      timeout: 5000,
    });

    // Reload the page
    await page.reload();

    // Unlock
    const unlockBtn = page.locator("#unlock-btn");
    await unlockBtn.waitFor({ state: "visible" });
    await unlockBtn.click();
    await page
      .locator("#unlock-overlay")
      .waitFor({ state: "hidden", timeout: 10000 });

    // Wait for posts to load (same count as before reload)
    await expect(
      postList.locator('[data-testid="test-post-wrapper"]'),
    ).toHaveCount(countBefore + 1, { timeout: 10000 });

    // Click on the post we created
    await postList
      .locator('[data-testid="test-post-item"]')
      .filter({ hasText: "Persist test content" })
      .click();

    // Verify content persisted
    await expect(editor).toContainText("Persist test content", {
      timeout: 5000,
    });
  });

  test("delete button is enabled after post loads", async () => {
    const { page } = userResult;

    const postList = page.locator("#post-list");
    const deleteBtn = page.locator("#delete-btn");

    // Get all posts and switch between first two
    const postItems = postList.locator('[data-testid="test-post-item"]');
    const postCount = await postItems.count();
    expect(postCount).toBeGreaterThanOrEqual(2);

    // Switch to first post
    await postItems.first().click();

    // After post is loaded, delete button should be enabled
    await expect(deleteBtn).toBeEnabled({ timeout: 5000 });

    // Switch to second post
    await postItems.nth(1).click();

    // After loading completes, delete button should be enabled again
    await expect(deleteBtn).toBeEnabled({ timeout: 5000 });
  });

  test("new post button creates new post", async () => {
    const { page } = userResult;

    const postList = page.locator("#post-list");
    const newPostBtn = page.locator("#new-post-btn");
    const initialCount = await postList
      .locator('[data-testid="test-post-wrapper"]')
      .count();

    // Create a new post
    await newPostBtn.click();

    // Should have one more post
    await expect(
      postList.locator('[data-testid="test-post-wrapper"]'),
    ).toHaveCount(initialCount + 1, { timeout: 5000 });

    // Button should still be clickable
    await expect(newPostBtn).toBeEnabled();
  });

  test("deleting current post selects another post", async () => {
    const { page } = userResult;

    const postList = page.locator("#post-list");
    const editor = page.locator('[data-testid="test-editor-content"]');
    const deleteBtn = page.locator("#delete-btn");

    // Create a new post with specific content
    await page.locator("#new-post-btn").click();
    await page.waitForTimeout(500);

    await editor.click();
    await page.keyboard.type("Delete me post");

    // Wait for title to appear
    await expect(
      postList
        .locator('[data-testid="test-post-item"]')
        .filter({ hasText: "Delete me post" }),
    ).toBeVisible({ timeout: 5000 });

    const countBeforeDelete = await postList
      .locator('[data-testid="test-post-wrapper"]')
      .count();

    // Delete the current post
    page.once("dialog", (dialog) => dialog.accept());
    await deleteBtn.click();

    // Should have one less post
    await expect(
      postList.locator('[data-testid="test-post-wrapper"]'),
    ).toHaveCount(countBeforeDelete - 1, { timeout: 5000 });

    // The deleted post should not exist
    await expect(
      postList
        .locator('[data-testid="test-post-item"]')
        .filter({ hasText: "Delete me post" }),
    ).not.toBeVisible();

    // Another post should be selected (editor should have some content)
    // Wait a bit for the selection to happen
    await page.waitForTimeout(1000);

    // Check that some post is now active
    const activePost = postList.locator(
      '[data-testid="test-post-item"].active',
    );
    await expect(activePost).toBeVisible({ timeout: 5000 });
  });

  test("undo history is cleared when switching posts", async () => {
    const { page } = userResult;

    const postList = page.locator("#post-list");
    const editor = page.locator('[data-testid="test-editor-content"]');

    // Ensure we have at least 2 posts
    let postCount = await postList
      .locator('[data-testid="test-post-wrapper"]')
      .count();

    while (postCount < 2) {
      await page.locator("#new-post-btn").click();
      await page.waitForTimeout(500);
      await editor.click();
      await page.keyboard.type(`Undo test post ${postCount + 1}`);
      await page.waitForTimeout(500);
      postCount = await postList
        .locator('[data-testid="test-post-wrapper"]')
        .count();
    }

    const postItems = postList.locator('[data-testid="test-post-item"]');

    // Go to first post and record its content
    await postItems.first().click();
    await page.waitForTimeout(500);
    const firstPostContent = await editor.textContent();

    // Go to second post and add unique text
    await postItems.nth(1).click();
    await page.waitForTimeout(500);

    await editor.click();
    await page.keyboard.press("End");
    await page.keyboard.type("UNIQUE_MARKER_FOR_UNDO_TEST");

    // Verify the text was added
    await expect(editor).toContainText("UNIQUE_MARKER_FOR_UNDO_TEST", {
      timeout: 2000,
    });

    // Switch back to first post
    await postItems.first().click();
    await page.waitForTimeout(500);

    // Verify we're on first post (should contain first post content)
    await expect(editor).toContainText(firstPostContent || "", {
      timeout: 5000,
    });

    // Verify undo marker is NOT present (we switched posts)
    const contentAfterSwitch = await editor.textContent();
    expect(contentAfterSwitch).not.toContain("UNIQUE_MARKER_FOR_UNDO_TEST");

    // Try to undo - should NOT bring back content from second post
    await page.keyboard.press("Meta+z");
    await page.waitForTimeout(500);

    // The content should still not contain the undo marker
    const contentAfterUndo = await editor.textContent();
    expect(contentAfterUndo).not.toContain("UNIQUE_MARKER_FOR_UNDO_TEST");
  });
});

test.describe("Post navigation - sync behavior", () => {
  let context: BrowserContext;
  let userResult: CreateUserResult;
  let baseUrl: string;

  test.beforeAll(async ({ browser }) => {
    const server = await getServer(Server.Default);
    baseUrl = server.baseUrl;
    context = await browser.newContext();
    const username = `postsave_${uniqueTestId()}`;
    userResult = await createUser({
      context,
      baseUrl,
      username,
      enableEncryption: true,
    });
  });

  test.afterAll(async () => {
    await context?.close();
  });

  test("sync indicator shows idle when no changes", async () => {
    const { page } = userResult;

    // Wait for initial post and editor
    const postList = page.locator("#post-list");
    await expect(
      postList.locator('[data-testid="test-post-wrapper"]'),
    ).toHaveCount(1, { timeout: 10000 });

    const syncIndicator = page.locator('[data-testid="test-sync-indicator"]');

    // Initially should show idle state
    await expect(syncIndicator).toHaveAttribute("data-status", "idle", {
      timeout: 5000,
    });
  });

  test("sync indicator shows pending when there are changes", async () => {
    const { page } = userResult;

    const editor = page.locator('[data-testid="test-editor-content"]');
    const syncIndicator = page.locator('[data-testid="test-sync-indicator"]');

    // Type something
    await editor.click();
    await page.keyboard.type("Making changes for sync test");

    // Sync indicator should change to "pending"
    await expect(syncIndicator).toHaveAttribute("data-status", "pending", {
      timeout: 5000,
    });
  });

  test("force save button triggers sync and shows synced", async () => {
    const { page } = userResult;

    const editor = page.locator('[data-testid="test-editor-content"]');
    const syncIndicator = page.locator('[data-testid="test-sync-indicator"]');
    const forceSaveBtn = page.locator('[data-testid="test-force-save-btn"]');

    // Make a change first to ensure there's something to save
    await editor.click();
    await page.keyboard.type(" more text");

    // Should be pending
    await expect(syncIndicator).toHaveAttribute("data-status", "pending", {
      timeout: 5000,
    });

    // Click force save
    await forceSaveBtn.click();

    // Should show synced briefly
    await expect(syncIndicator).toHaveAttribute("data-status", "synced", {
      timeout: 5000,
    });

    // Then return to idle
    await expect(syncIndicator).toHaveAttribute("data-status", "idle", {
      timeout: 5000,
    });
  });

  test("switching posts auto-saves current post", async () => {
    const { page } = userResult;

    const postList = page.locator("#post-list");
    const editor = page.locator('[data-testid="test-editor-content"]');

    // Create a new post
    await page.locator("#new-post-btn").click();
    await expect(
      postList.locator('[data-testid="test-post-wrapper"]'),
    ).toHaveCount(2, { timeout: 5000 });

    // Type in the new post
    await editor.click();
    await page.keyboard.type("Auto-save test content");

    // Wait for title to appear
    await expect(
      postList
        .locator('[data-testid="test-post-item"]')
        .filter({ hasText: "Auto-save test content" }),
    ).toBeVisible({ timeout: 5000 });

    // Switch to first post (this should auto-save)
    const firstPost = postList
      .locator('[data-testid="test-post-item"]')
      .first();
    await firstPost.click();

    // Wait for the switch to complete (first post should be loaded)
    await expect(editor).toBeVisible({ timeout: 5000 });

    // Reload and verify the content was saved
    await page.reload();

    // Unlock
    const unlockBtn = page.locator("#unlock-btn");
    await unlockBtn.waitFor({ state: "visible" });
    await unlockBtn.click();
    await page
      .locator("#unlock-overlay")
      .waitFor({ state: "hidden", timeout: 10000 });

    // Wait for posts
    await expect(
      postList.locator('[data-testid="test-post-wrapper"]'),
    ).toHaveCount(2, { timeout: 10000 });

    // Click on the auto-saved post
    await postList
      .locator('[data-testid="test-post-item"]')
      .filter({ hasText: "Auto-save test content" })
      .click();

    // Verify content was saved
    await expect(editor).toContainText("Auto-save test content", {
      timeout: 5000,
    });
  });
});

test.describe("Post navigation - editor state", () => {
  let context: BrowserContext;
  let userResult: CreateUserResult;
  let baseUrl: string;

  test.beforeAll(async ({ browser }) => {
    const server = await getServer(Server.Default);
    baseUrl = server.baseUrl;
    context = await browser.newContext();
    const username = `editorstate_${uniqueTestId()}`;
    userResult = await createUser({
      context,
      baseUrl,
      username,
      enableEncryption: true,
    });
  });

  test.afterAll(async () => {
    await context?.close();
  });

  test("editor is editable after post loads", async () => {
    const { page } = userResult;

    const postList = page.locator("#post-list");
    const editor = page.locator('[data-testid="test-editor-content"]');

    // Wait for initial post
    await expect(
      postList.locator('[data-testid="test-post-wrapper"]'),
    ).toHaveCount(1, { timeout: 10000 });

    // Editor should be editable
    await expect(editor).toHaveAttribute("contenteditable", "true");

    // Should be able to type
    await editor.click();
    await page.keyboard.type("Editor is editable");

    // Content should appear
    await expect(editor).toContainText("Editor is editable", { timeout: 2000 });
  });

  test("editor content is replaced not appended when switching posts", async () => {
    const { page } = userResult;

    const postList = page.locator("#post-list");
    const editor = page.locator('[data-testid="test-editor-content"]');

    // Create second post
    await page.locator("#new-post-btn").click();
    await expect(
      postList.locator('[data-testid="test-post-wrapper"]'),
    ).toHaveCount(2, { timeout: 5000 });

    // Type in second post
    await editor.click();
    await page.keyboard.type("Second post only");

    // Wait for title
    await expect(
      postList
        .locator('[data-testid="test-post-item"]')
        .filter({ hasText: "Second post only" }),
    ).toBeVisible({ timeout: 5000 });

    // Switch to first post
    const firstPost = postList
      .locator('[data-testid="test-post-item"]')
      .filter({ hasText: "Editor is editable" });
    await firstPost.click();

    // Wait for content to load
    await expect(editor).toContainText("Editor is editable", { timeout: 5000 });

    // Content should NOT contain second post content (not appended)
    const content = await editor.textContent();
    expect(content).not.toContain("Second post only");
  });

  test("editor regains focus behavior after post switch", async () => {
    const { page } = userResult;

    const postList = page.locator("#post-list");
    const editor = page.locator('[data-testid="test-editor-content"]');

    // Get the two posts
    const firstPost = postList
      .locator('[data-testid="test-post-item"]')
      .filter({ hasText: "Editor is editable" });
    const secondPost = postList
      .locator('[data-testid="test-post-item"]')
      .filter({ hasText: "Second post only" });

    // Switch to second post
    await secondPost.click();
    await expect(editor).toContainText("Second post only", { timeout: 5000 });

    // Click on editor and type
    await editor.click();
    await page.keyboard.press("End");
    await page.keyboard.type(" - can still type");

    // Verify typing worked
    await expect(editor).toContainText("can still type", { timeout: 2000 });

    // Switch to first post
    await firstPost.click();
    await expect(editor).toContainText("Editor is editable", { timeout: 5000 });

    // Click on editor and type
    await editor.click();
    await page.keyboard.press("End");
    await page.keyboard.type(" - also works here");

    // Verify typing worked
    await expect(editor).toContainText("also works here", { timeout: 2000 });
  });
});
