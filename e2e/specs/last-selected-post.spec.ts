import {
  test,
  expect,
  createUser,
  uniqueTestId,
  CreateUserResult,
} from "../utils/fixtures.ts";
import { getServer, Server } from "../utils/server.ts";
import { BrowserContext } from "@playwright/test";

/**
 * Helper to save the current post using force save button.
 */
async function forceSave(page: import("@playwright/test").Page): Promise<void> {
  const syncIndicator = page.locator('[data-testid="test-sync-indicator"]');
  const forceSaveBtn = page.locator('[data-testid="test-force-save-btn"]');

  const status = await syncIndicator.getAttribute("data-status");
  if (status === "pending") {
    await forceSaveBtn.click();
    await expect(syncIndicator).toHaveAttribute("data-status", "synced", {
      timeout: 5000,
    });
  }
}

/**
 * Helper to create a post with a specific title.
 */
async function createPostWithTitle(
  page: import("@playwright/test").Page,
  title: string,
): Promise<void> {
  const newPostBtn = page.locator("#new-post-btn");
  await newPostBtn.click();

  const editor = page.locator('[data-testid="test-editor-content"]');
  await editor.click();
  await page.keyboard.type(title);

  const postList = page.locator("#post-list");
  await expect(
    postList
      .locator('[data-testid="test-post-item"]')
      .filter({ hasText: title }),
  ).toBeVisible({ timeout: 5000 });

  // Save the post
  await forceSave(page);
}

/**
 * Helper to get a post wrapper by title.
 */
function getPostByTitle(page: import("@playwright/test").Page, title: string) {
  const postList = page.locator("#post-list");
  return postList.locator('[data-testid="test-post-wrapper"]').filter({
    has: page.locator('[data-testid="test-post-item"]', { hasText: title }),
  });
}

/**
 * Helper to drag one post onto another (for reparenting).
 */
async function dragPostToPost(
  page: import("@playwright/test").Page,
  sourceTitle: string,
  targetTitle: string,
): Promise<void> {
  const sourcePost = getPostByTitle(page, sourceTitle);
  const targetPost = getPostByTitle(page, targetTitle);

  const sourceBox = await sourcePost.boundingBox();
  const targetBox = await targetPost.boundingBox();

  if (!sourceBox || !targetBox) {
    throw new Error("Could not get bounding boxes for drag and drop");
  }

  const targetY = targetBox.y + targetBox.height / 2; // Center = reparent

  await page.mouse.move(
    sourceBox.x + sourceBox.width / 2,
    sourceBox.y + sourceBox.height / 2,
  );
  await page.mouse.down();
  await page.mouse.move(targetBox.x + targetBox.width / 2, targetY, {
    steps: 10,
  });
  await page.mouse.up();
}

/**
 * Helper to reload and unlock the page.
 */
async function reloadAndUnlock(
  page: import("@playwright/test").Page,
): Promise<void> {
  await page.reload();
  const unlockBtn = page.locator("#unlock-btn");
  await unlockBtn.waitFor({ state: "visible" });
  await unlockBtn.click();
  const unlockOverlay = page.locator("#unlock-overlay");
  await unlockOverlay.waitFor({ state: "hidden", timeout: 10000 });
}

test.describe("Last selected post persistence", () => {
  let context: BrowserContext;
  let userResult: CreateUserResult;
  let baseUrl: string;

  test.beforeAll(async ({ browser }) => {
    const server = await getServer(Server.Default);
    baseUrl = server.baseUrl;
    context = await browser.newContext();
    const username = `lastpost_${uniqueTestId()}`;
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

  test("remembers last selected post after page reload", async () => {
    const { page } = userResult;
    const postList = page.locator("#post-list");
    const editor = page.locator('[data-testid="test-editor-content"]');

    // Wait for initial post
    await expect(
      postList.locator('[data-testid="test-post-wrapper"]'),
    ).toHaveCount(1, { timeout: 10000 });

    // Create two posts
    await createPostWithTitle(page, "First Post");
    await createPostWithTitle(page, "Second Post");

    // Wait for posts
    await expect(
      postList.locator('[data-testid="test-post-wrapper"]'),
    ).toHaveCount(3, { timeout: 5000 });

    // Select the first named post
    await postList
      .locator('[data-testid="test-post-item"]')
      .filter({ hasText: "First Post" })
      .click();

    // Verify it's selected
    await expect(editor).toContainText("First Post", { timeout: 5000 });

    // Save before reload
    await forceSave(page);

    // Reload page
    await reloadAndUnlock(page);

    // Wait for posts to load
    await expect(
      postList.locator('[data-testid="test-post-wrapper"]'),
    ).toHaveCount(3, { timeout: 10000 });

    // Verify "First Post" is selected (active and in editor)
    const activePost = postList.locator(
      '[data-testid="test-post-item"].active',
    );
    await expect(activePost).toContainText("First Post", { timeout: 5000 });
    await expect(editor).toContainText("First Post", { timeout: 5000 });
  });

  test("remembers nested child post after reload", async () => {
    const { page } = userResult;
    const postList = page.locator("#post-list");
    const editor = page.locator('[data-testid="test-editor-content"]');

    // Create parent and child posts
    await createPostWithTitle(page, "Parent Post");
    await createPostWithTitle(page, "Child Post");

    // Wait for both posts to be visible
    await expect(
      postList
        .locator('[data-testid="test-post-item"]')
        .filter({ hasText: "Parent Post" }),
    ).toBeVisible({ timeout: 5000 });
    await expect(
      postList
        .locator('[data-testid="test-post-item"]')
        .filter({ hasText: "Child Post" }),
    ).toBeVisible({ timeout: 5000 });

    // Create nested structure: Child under Parent
    await dragPostToPost(page, "Child Post", "Parent Post");

    // Verify Child is at depth 1 (nested under parent)
    const childWrapper = getPostByTitle(page, "Child Post");
    await expect(childWrapper).toHaveAttribute("data-depth", "1");

    // Select Child (the nested post)
    await postList
      .locator('[data-testid="test-post-item"]')
      .filter({ hasText: "Child Post" })
      .click();
    await expect(editor).toContainText("Child Post", { timeout: 5000 });

    // Save before reload
    await forceSave(page);

    // Wait for reparent API call to complete before reloading
    await page.waitForLoadState("networkidle");

    // Reload page
    await reloadAndUnlock(page);

    // Wait for a post to be selected (loadPosts renders first, then selects async)
    const activePost = postList.locator(
      '[data-testid="test-post-item"].active',
    );
    await expect(activePost).toBeVisible({ timeout: 10000 });

    // The nested post should be selected and parent expanded
    await expect(activePost).toContainText("Child Post", { timeout: 5000 });
    await expect(editor).toContainText("Child Post", { timeout: 5000 });

    // Verify the post is visible (parent was expanded)
    await expect(
      postList
        .locator('[data-testid="test-post-item"]')
        .filter({ hasText: "Child Post" }),
    ).toBeVisible();
  });

  test("falls back to first post if last selected was deleted", async () => {
    const { page } = userResult;
    const postList = page.locator("#post-list");
    const editor = page.locator('[data-testid="test-editor-content"]');

    // Create a post that we'll delete
    await createPostWithTitle(page, "Will Be Deleted");

    // Wait for post
    await expect(
      postList
        .locator('[data-testid="test-post-item"]')
        .filter({ hasText: "Will Be Deleted" }),
    ).toBeVisible({ timeout: 5000 });

    // Select this post
    await postList
      .locator('[data-testid="test-post-item"]')
      .filter({ hasText: "Will Be Deleted" })
      .click();
    await expect(editor).toContainText("Will Be Deleted", { timeout: 5000 });

    // Save before deletion
    await forceSave(page);

    // Delete this post
    const deleteBtn = page.locator("#delete-btn");
    page.once("dialog", (dialog) => dialog.accept());
    await deleteBtn.click();

    // Wait for deletion
    await expect(
      postList
        .locator('[data-testid="test-post-item"]')
        .filter({ hasText: "Will Be Deleted" }),
    ).not.toBeVisible({ timeout: 5000 });

    // Reload - should select first available post instead of the deleted one
    await reloadAndUnlock(page);

    // Wait for posts to load
    await expect(
      postList.locator('[data-testid="test-post-wrapper"]').first(),
    ).toBeVisible({ timeout: 10000 });

    // Some post should be selected (the first one)
    const activePost = postList.locator(
      '[data-testid="test-post-item"].active',
    );
    await expect(activePost).toBeVisible({ timeout: 5000 });

    // The deleted post should not be selected
    await expect(activePost).not.toContainText("Will Be Deleted");
  });
});
