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

test.describe("Nested posts - Visual hierarchy", () => {
  test.describe.configure({ mode: "serial" });

  let context: BrowserContext;
  let userResult: CreateUserResult;
  let baseUrl: string;

  test.beforeAll(async ({ browser }) => {
    const server = await getServer(Server.Default);
    baseUrl = server.baseUrl;
    context = await browser.newContext();
    const username = `nested_visual_${uniqueTestId()}`;
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

  test("nested posts have correct indentation", async () => {
    const { page } = userResult;
    const postList = page.locator("#post-list");

    // Wait for initial post
    await expect(
      postList.locator('[data-testid="test-post-wrapper"]'),
    ).toHaveCount(1, {
      timeout: 10000,
    });

    // Create hierarchy: Root -> Level 1 -> Level 2
    await createPostWithTitle(page, "Indent Root");
    await createPostWithTitle(page, "Indent Level 1");
    await createPostWithTitle(page, "Indent Level 2");

    // Drag Level 1 under Root
    await dragPostToPost(page, "Indent Level 1", "Indent Root", {
      centerTarget: true,
    });

    // Wait for Level 1 to be nested (depth 1)
    const level1Post = getPostByTitle(page, "Indent Level 1");
    await expect(level1Post).toHaveAttribute("data-depth", "1", {
      timeout: 5000,
    });

    // Drag Level 2 under Level 1
    await dragPostToPost(page, "Indent Level 2", "Indent Level 1", {
      centerTarget: true,
    });

    // Wait for Level 2 to be nested (depth 2)
    const level2Post = getPostByTitle(page, "Indent Level 2");
    await expect(level2Post).toHaveAttribute("data-depth", "2", {
      timeout: 5000,
    });

    // Verify indentation via padding-left style
    const root = getPostByTitle(page, "Indent Root");
    const rootContainer = root.locator(
      '[data-testid="test-post-item-container"]',
    );
    const level1Container = level1Post.locator(
      '[data-testid="test-post-item-container"]',
    );
    const level2Container = level2Post.locator(
      '[data-testid="test-post-item-container"]',
    );

    // Root (depth 0) should have 0px padding
    const rootPadding = await rootContainer.evaluate(
      (el) => getComputedStyle(el).paddingLeft,
    );
    expect(rootPadding).toBe("0px");

    // Level 1 (depth 1) should have 16px padding
    const level1Padding = await level1Container.evaluate(
      (el) => getComputedStyle(el).paddingLeft,
    );
    expect(level1Padding).toBe("16px");

    // Level 2 (depth 2) should have 32px padding
    const level2Padding = await level2Container.evaluate(
      (el) => getComputedStyle(el).paddingLeft,
    );
    expect(level2Padding).toBe("32px");
  });

  test("chevron toggles when expanded/collapsed", async () => {
    const { page } = userResult;

    // Use the hierarchy we just created
    const root = getPostByTitle(page, "Indent Root");
    const expandBtn = root.locator('[data-testid="test-post-expand-btn"]');

    // Verify chevron element exists
    const chevron = expandBtn.locator('[data-testid="test-chevron"]');
    await expect(chevron).toBeVisible();

    // Ensure expanded initially
    await expect(expandBtn).toHaveAttribute("data-expanded", "true");

    // Collapse
    await expandBtn.click();
    await expect(expandBtn).toHaveAttribute("data-expanded", "false");

    // Expand again
    await expandBtn.click();
    await expect(expandBtn).toHaveAttribute("data-expanded", "true");
  });
});
