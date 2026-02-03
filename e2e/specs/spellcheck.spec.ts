import {
  test,
  expect,
  createUser,
  uniqueTestId,
  CreateUserResult,
} from "../utils/fixtures.ts";
import { getServer, Server } from "../utils/server.ts";
import { BrowserContext } from "@playwright/test";

test.describe("Spellcheck toggle", () => {
  let context: BrowserContext;
  let userResult: CreateUserResult;
  let baseUrl: string;

  test.beforeAll(async ({ browser }) => {
    const server = await getServer(Server.Default);
    baseUrl = server.baseUrl;
    context = await browser.newContext();
    const username = `spellcheck_${uniqueTestId()}`;
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

  test("spellcheck button toggles state", async () => {
    const { page } = userResult;

    // Wait for spellcheck button to be visible
    const spellcheckBtn = page.locator("#spellcheck-btn");
    await expect(spellcheckBtn).toBeVisible({ timeout: 5000 });

    // Initially spellcheck should be disabled
    await expect(spellcheckBtn).toHaveAttribute("data-enabled", "false");

    // Click to enable spellcheck
    await spellcheckBtn.click();

    // Verify spellcheck is now enabled
    await expect(spellcheckBtn).toHaveAttribute("data-enabled", "true");

    // Click again to disable
    await spellcheckBtn.click();

    // Verify spellcheck is disabled again
    await expect(spellcheckBtn).toHaveAttribute("data-enabled", "false");
  });

  test("spellcheck state persists after page reload", async () => {
    const { page } = userResult;

    const spellcheckBtn = page.locator("#spellcheck-btn");
    await expect(spellcheckBtn).toBeVisible({ timeout: 5000 });

    // Ensure spellcheck is disabled first
    const currentState = await spellcheckBtn.getAttribute("data-enabled");
    if (currentState === "true") {
      await spellcheckBtn.click();
    }
    await expect(spellcheckBtn).toHaveAttribute("data-enabled", "false");

    // Enable spellcheck
    await spellcheckBtn.click();
    await expect(spellcheckBtn).toHaveAttribute("data-enabled", "true");

    // Reload the page
    await page.reload();

    // Wait for unlock overlay if it appears, and unlock
    const unlockOverlay = page.locator("#unlock-overlay");
    if (await unlockOverlay.isVisible()) {
      const unlockBtn = page.locator("#unlock-btn");
      await unlockBtn.click();
      await unlockOverlay.waitFor({ state: "hidden", timeout: 10000 });
    }

    // Verify spellcheck is still enabled after reload
    await expect(spellcheckBtn).toBeVisible({ timeout: 5000 });
    await expect(spellcheckBtn).toHaveAttribute("data-enabled", "true");

    // Clean up: disable spellcheck
    await spellcheckBtn.click();
    await expect(spellcheckBtn).toHaveAttribute("data-enabled", "false");
  });

  test("spellcheck applies to editor when post is selected", async () => {
    const { page } = userResult;

    // Create a new post
    const newPostBtn = page.locator("#new-post-btn");
    await newPostBtn.click();

    // Wait for editor to be ready
    const editor = page.locator('[data-testid="test-editor-content"]');
    await expect(editor).toBeVisible({ timeout: 5000 });

    // Enable spellcheck
    const spellcheckBtn = page.locator("#spellcheck-btn");
    await spellcheckBtn.click();
    await expect(spellcheckBtn).toHaveAttribute("data-enabled", "true");

    // Verify spellcheck attribute is set on editor content
    await expect(editor).toHaveAttribute("spellcheck", "true");

    // Disable spellcheck
    await spellcheckBtn.click();
    await expect(spellcheckBtn).toHaveAttribute("data-enabled", "false");

    // Verify spellcheck attribute is removed from editor content
    await expect(editor).toHaveAttribute("spellcheck", "false");
  });

  test("delete and spellcheck buttons hidden when sidebar expanded on mobile", async () => {
    const { page } = userResult;

    // Set viewport to mobile size
    await page.setViewportSize({ width: 600, height: 800 });

    const sidebar = page.locator("#sidebar");
    const deleteBtn = page.locator("#delete-btn");
    const syncIndicator = page.locator('[data-testid="test-sync-indicator"]');
    const spellcheckBtn = page.locator("#spellcheck-btn");
    const toggleBtn = page.locator("#sidebar-toggle");

    // Ensure sidebar is expanded (not collapsed)
    if ((await sidebar.getAttribute("data-collapsed")) !== null) {
      await toggleBtn.click();
    }
    await expect(sidebar).not.toHaveAttribute("data-collapsed");

    // Delete and spellcheck buttons should be hidden when sidebar is expanded on mobile
    await expect(deleteBtn).toBeHidden();
    await expect(spellcheckBtn).toBeHidden();

    // Sync indicator should still be visible
    await expect(syncIndicator).toBeVisible();

    // Collapse sidebar
    await toggleBtn.click();
    await expect(sidebar).toHaveAttribute("data-collapsed");

    // Now all buttons should be visible
    await expect(deleteBtn).toBeVisible();
    await expect(syncIndicator).toBeVisible();
    await expect(spellcheckBtn).toBeVisible();
  });
});
