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

test.describe("Post reorder functionality", () => {
  let context: BrowserContext;
  let userResult: CreateUserResult;
  let baseUrl: string;

  test.beforeAll(async ({ browser }) => {
    const server = await getServer(Server.Default);
    baseUrl = server.baseUrl;
    context = await browser.newContext();
    const username = `reorder_${uniqueTestId()}`;
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

  test("can reorder posts via drag and drop", async () => {
    const { page } = userResult;

    // Page is already at the app and unlocked from createUser
    // Wait for initial post (created automatically on first visit)
    const postList = page.locator("#post-list");
    await expect(
      postList.locator('[data-testid="test-post-wrapper"]'),
    ).toHaveCount(1, {
      timeout: 10000,
    });

    // Create a second post by clicking the new post button
    const newPostBtn = page.locator("#new-post-btn");
    await newPostBtn.click();

    // Wait for second post to appear
    await expect(
      postList.locator('[data-testid="test-post-wrapper"]'),
    ).toHaveCount(2, {
      timeout: 5000,
    });

    // Type content in the second post to give it a title
    const editor = page.locator("#editor .cm-content");
    await editor.click();
    await page.keyboard.type("Second Post");

    // Wait for title to update in sidebar
    await expect(
      postList
        .locator('[data-testid="test-post-item"]')
        .filter({ hasText: "Second Post" }),
    ).toBeVisible({ timeout: 5000 });

    // Save before creating next post
    await forceSave(page);

    // Create a third post
    await newPostBtn.click();
    await expect(
      postList.locator('[data-testid="test-post-wrapper"]'),
    ).toHaveCount(3, {
      timeout: 5000,
    });

    // Type content in the third post
    await editor.click();
    await page.keyboard.type("Third Post");

    // Wait for title to update in sidebar
    await expect(
      postList
        .locator('[data-testid="test-post-item"]')
        .filter({ hasText: "Third Post" }),
    ).toBeVisible({ timeout: 5000 });

    // Save before checking order
    await forceSave(page);

    // Get the initial order - newest posts are at the top
    // Order should be: Third Post, Second Post, Untitled
    const postWrappers = postList.locator('[data-testid="test-post-wrapper"]');
    const initialFirstPost = await postWrappers
      .nth(0)
      .locator('[data-testid="test-post-item"]')
      .textContent();
    const initialSecondPost = await postWrappers
      .nth(1)
      .locator('[data-testid="test-post-item"]')
      .textContent();
    const initialThirdPost = await postWrappers
      .nth(2)
      .locator('[data-testid="test-post-item"]')
      .textContent();

    expect(initialFirstPost).toBe("Third Post");
    expect(initialSecondPost).toBe("Second Post");
    expect(initialThirdPost).toBe("Untitled");

    // Perform drag and drop: move Third Post (index 0) to after Second Post (index 1)
    const sourcePost = postWrappers.nth(0);
    const targetPost = postWrappers.nth(1);

    // Get bounding boxes
    const sourceBox = await sourcePost.boundingBox();
    const targetBox = await targetPost.boundingBox();

    if (!sourceBox || !targetBox) {
      throw new Error("Could not get bounding boxes for drag and drop");
    }

    // Drag from center of source to bottom edge of target
    await page.mouse.move(
      sourceBox.x + sourceBox.width / 2,
      sourceBox.y + sourceBox.height / 2,
    );
    await page.mouse.down();

    // Move to bottom half of target post
    await page.mouse.move(
      targetBox.x + targetBox.width / 2,
      targetBox.y + targetBox.height * 0.75,
      { steps: 10 },
    );

    await page.mouse.up();

    // Wait for reorder to complete by checking the new order
    await expect(
      postWrappers.nth(0).locator('[data-testid="test-post-item"]'),
    ).toHaveText("Second Post", { timeout: 5000 });

    // Verify the new order: Second Post, Third Post, Untitled
    const newFirstPost = await postWrappers
      .nth(0)
      .locator('[data-testid="test-post-item"]')
      .textContent();
    const newSecondPost = await postWrappers
      .nth(1)
      .locator('[data-testid="test-post-item"]')
      .textContent();
    const newThirdPost = await postWrappers
      .nth(2)
      .locator('[data-testid="test-post-item"]')
      .textContent();

    expect(newFirstPost).toBe("Second Post");
    expect(newSecondPost).toBe("Third Post");
    expect(newThirdPost).toBe("Untitled");

    // Reload the page to verify persistence
    await page.reload();

    // Unlock the posts (encryption is enabled)
    const unlockBtn = page.locator("#unlock-btn");
    await unlockBtn.waitFor({ state: "visible" });
    await unlockBtn.click();

    // Wait for unlock to complete
    const unlockOverlay = page.locator("#unlock-overlay");
    await unlockOverlay.waitFor({ state: "hidden", timeout: 10000 });

    // Wait for posts to load
    await expect(
      postList.locator('[data-testid="test-post-wrapper"]'),
    ).toHaveCount(3, {
      timeout: 10000,
    });

    // Verify the order persisted after reload
    const reloadedFirstPost = await postWrappers
      .nth(0)
      .locator('[data-testid="test-post-item"]')
      .textContent();
    const reloadedSecondPost = await postWrappers
      .nth(1)
      .locator('[data-testid="test-post-item"]')
      .textContent();
    const reloadedThirdPost = await postWrappers
      .nth(2)
      .locator('[data-testid="test-post-item"]')
      .textContent();

    expect(reloadedFirstPost).toBe("Second Post");
    expect(reloadedSecondPost).toBe("Third Post");
    expect(reloadedThirdPost).toBe("Untitled");
  });
});
