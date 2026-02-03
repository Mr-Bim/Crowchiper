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
  getPostByTitle,
  dragPostToPost,
} from "./nested-posts-helpers.ts";

test.describe("Nested posts - Expand/Collapse", () => {
  test.describe.configure({ mode: "serial" });

  let context: BrowserContext;
  let userResult: CreateUserResult;
  let baseUrl: string;

  test.beforeAll(async ({ browser }) => {
    const server = await getServer(Server.Default);
    baseUrl = server.baseUrl;
    context = await browser.newContext();
    const username = `nested_expand_${uniqueTestId()}`;
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

  test("can expand and collapse nested posts", async () => {
    const { page } = userResult;
    const postList = page.locator("#post-list");

    // Wait for initial post
    await expect(
      postList.locator('[data-testid="test-post-wrapper"]'),
    ).toHaveCount(1, {
      timeout: 10000,
    });

    // Create parent and child
    await createPostWithTitle(page, "Expand Parent");
    await createPostWithTitle(page, "Expand Child");

    // Drag child under parent
    await dragPostToPost(page, "Expand Child", "Expand Parent", {
      centerTarget: true,
    });

    // Parent should be expanded after reparenting
    const parentPost = getPostByTitle(page, "Expand Parent");
    const expandBtn = parentPost.locator(
      '[data-testid="test-post-expand-btn"]',
    );
    await expect(expandBtn).toBeVisible();
    await expect(expandBtn).toHaveAttribute("data-post-expanded", "true");

    // Child should be visible
    const childUpdated = getPostByTitle(page, "Expand Child");
    await expect(childUpdated).toBeVisible();

    // Click expand button to collapse
    await expandBtn.click();
    await page.waitForTimeout(300);

    // Expand button should now show collapsed state
    await expect(expandBtn).toHaveAttribute("data-post-expanded", "false");

    // Child should be hidden (not visible in DOM)
    await expect(childUpdated).not.toBeVisible();

    // Click expand button again to expand
    await expandBtn.click();
    await page.waitForTimeout(300);

    // Child should be visible again
    await expect(expandBtn).toHaveAttribute("data-post-expanded", "true");
    const childVisible = getPostByTitle(page, "Expand Child");
    await expect(childVisible).toBeVisible();
  });

  test("deeply nested posts are expandable", async () => {
    const { page } = userResult;

    // Create Level 1 parent
    await createPostWithTitle(page, "Level 1");

    // Create Level 2 and nest under Level 1
    await createPostWithTitle(page, "Level 2");
    await dragPostToPost(page, "Level 2", "Level 1", { centerTarget: true });

    // Create Level 3 and nest under Level 2
    await createPostWithTitle(page, "Level 3");
    await dragPostToPost(page, "Level 3", "Level 2", { centerTarget: true });

    // Verify depths
    const level1 = getPostByTitle(page, "Level 1");
    const level2 = getPostByTitle(page, "Level 2");
    const level3 = getPostByTitle(page, "Level 3");

    await expect(level1).toHaveAttribute("data-depth", "0");
    await expect(level2).toHaveAttribute("data-depth", "1");
    await expect(level3).toHaveAttribute("data-depth", "2");

    // Both Level 1 and Level 2 should have expand buttons
    const level1ExpandBtn = level1.locator(
      '[data-testid="test-post-expand-btn"]',
    );
    const level2ExpandBtn = level2.locator(
      '[data-testid="test-post-expand-btn"]',
    );

    await expect(level1ExpandBtn).toBeVisible();
    await expect(level2ExpandBtn).toBeVisible();

    // Collapse Level 2 - Level 3 should hide
    await level2ExpandBtn.click();
    await page.waitForTimeout(300);
    await expect(level3).not.toBeVisible();

    // Collapse Level 1 - Level 2 should also hide
    await level1ExpandBtn.click();
    await page.waitForTimeout(300);
    await expect(level2).not.toBeVisible();

    // Expand Level 1 - Level 2 should appear but Level 3 still hidden
    await level1ExpandBtn.click();
    await page.waitForTimeout(300);
    const level2Visible = getPostByTitle(page, "Level 2");
    await expect(level2Visible).toBeVisible();
    // Level 3 stays hidden until Level 2 is expanded
    const level2ExpandBtnAgain = level2Visible.locator(
      '[data-testid="test-post-expand-btn"]',
    );
    await expect(level2ExpandBtnAgain).toHaveAttribute(
      "data-post-expanded",
      "false",
    );
  });
});
