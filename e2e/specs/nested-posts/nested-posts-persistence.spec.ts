import {
  test,
  expect,
  createUser,
  uniqueTestId,
  CreateUserResult,
} from "../../utils/fixtures.ts";
import { getServer, Server } from "../../utils/server.ts";
import { BrowserContext } from "@playwright/test";
import {
  createPostWithTitle,
  savePost,
  getPostByTitle,
  reloadAndUnlock,
  dragPostToPost,
} from "./nested-posts-helpers.ts";

test.describe("Nested posts - Persistence", () => {
  test.describe.configure({ mode: "serial" });

  let context: BrowserContext;
  let userResult: CreateUserResult;
  let baseUrl: string;

  test.beforeAll(async ({ browser }) => {
    const server = await getServer(Server.Default);
    baseUrl = server.baseUrl;
    context = await browser.newContext();
    const username = `nested_persist_${uniqueTestId()}`;
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

  test("nested structure persists after reload", async () => {
    const { page } = userResult;
    const postList = page.locator("#post-list");

    // Wait for initial post
    await expect(
      postList.locator('[data-testid="test-post-wrapper"]'),
    ).toHaveCount(1, {
      timeout: 10000,
    });

    // Create hierarchy
    await createPostWithTitle(page, "Persist Parent");
    await createPostWithTitle(page, "Persist Child");

    // Drag child under parent
    await dragPostToPost(page, "Persist Child", "Persist Parent", {
      centerTarget: true,
    });

    // Save
    await savePost(page);

    // Reload
    await reloadAndUnlock(page);

    // Wait for posts to load
    await expect(
      postList.locator('[data-testid="test-post-wrapper"]'),
    ).toHaveCount(3, {
      timeout: 10000,
    });

    // Verify hierarchy persisted
    const persistChild = getPostByTitle(page, "Persist Child");
    await expect(persistChild).toHaveAttribute("data-depth", "1");

    const persistParent = getPostByTitle(page, "Persist Parent");
    const parentUuid = await persistParent.getAttribute("data-post-uuid");
    await expect(persistChild).toHaveAttribute("data-parent-id", parentUuid!);

    // Parent should have expand button
    const expandBtn = persistParent.locator(
      '[data-testid="test-post-expand-btn"]',
    );
    await expect(expandBtn).toBeVisible();
  });

  test("expansion state resets to default on reload", async () => {
    const { page } = userResult;
    const postList = page.locator("#post-list");

    // Get parent and collapse it
    const parent = getPostByTitle(page, "Persist Parent");
    const expandBtn = parent.locator('[data-testid="test-post-expand-btn"]');

    // Collapse
    await expect(expandBtn).toHaveAttribute("data-post-expanded", "true");
    await expandBtn.click();
    await page.waitForTimeout(300);
    await expect(expandBtn).toHaveAttribute("data-post-expanded", "false");

    // Verify child is hidden
    const child = getPostByTitle(page, "Persist Child");
    await expect(child).not.toBeVisible();

    // Reload
    await reloadAndUnlock(page);

    // Wait for posts
    await expect(
      postList.locator('[data-testid="test-post-wrapper"]'),
    ).toHaveCount(3, {
      timeout: 10000,
    });

    // After reload, posts should be expanded to 3 levels by default
    const parentAfterReload = getPostByTitle(page, "Persist Parent");
    const expandBtnAfterReload = parentAfterReload.locator(
      '[data-testid="test-post-expand-btn"]',
    );
    await expect(expandBtnAfterReload).toHaveAttribute(
      "data-post-expanded",
      "true",
    );

    // Child should be visible
    const childAfterReload = getPostByTitle(page, "Persist Child");
    await expect(childAfterReload).toBeVisible();
  });
});
