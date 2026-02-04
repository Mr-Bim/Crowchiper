import {
  test,
  expect,
  createUser,
  uniqueTestId,
  CreateUserResult,
} from "../utils/fixtures.ts";
import { getServer, Server } from "../utils/server.ts";
import { BrowserContext } from "@playwright/test";

test.describe("New post as sibling", () => {
  let context: BrowserContext;
  let userResult: CreateUserResult;
  let baseUrl: string;

  test.beforeAll(async ({ browser }) => {
    const server = await getServer(Server.Default);
    baseUrl = server.baseUrl;
    context = await browser.newContext();
    const username = `newsibling_${uniqueTestId()}`;
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

  test("new post button creates post below current post", async () => {
    const { page } = userResult;

    const postList = page.locator("#post-list");
    const editor = page.locator('[data-testid="test-editor-content"]');
    const newPostBtn = page.locator("#new-post-btn");

    // Wait for initial post
    await expect(
      postList.locator('[data-testid="test-post-wrapper"]'),
    ).toHaveCount(1, { timeout: 10000 });

    // Name the first post
    await editor.click();
    await page.keyboard.type("First Post");

    // Wait for title to appear
    await expect(
      postList
        .locator('[data-testid="test-post-item"]')
        .filter({ hasText: "First Post" }),
    ).toBeVisible({ timeout: 5000 });

    // Create second post using button
    await newPostBtn.click();
    await expect(
      postList.locator('[data-testid="test-post-wrapper"]'),
    ).toHaveCount(2, { timeout: 5000 });

    // Name the second post
    await editor.click();
    await page.keyboard.type("Second Post");

    // Wait for title to appear
    await expect(
      postList
        .locator('[data-testid="test-post-item"]')
        .filter({ hasText: "Second Post" }),
    ).toBeVisible({ timeout: 5000 });

    // Verify order: First Post should be first, Second Post should be second
    const postItems = postList.locator('[data-testid="test-post-item"]');
    await expect(postItems.nth(0)).toContainText("First Post");
    await expect(postItems.nth(1)).toContainText("Second Post");

    // Now select the first post
    await postItems.nth(0).click();
    await expect(editor).toContainText("First Post", { timeout: 5000 });

    // Create a third post - should be inserted after First Post
    await newPostBtn.click();
    await expect(
      postList.locator('[data-testid="test-post-wrapper"]'),
    ).toHaveCount(3, { timeout: 5000 });

    // Name the third post
    await editor.click();
    await page.keyboard.type("Third Post (after First)");

    // Wait for title to appear
    await expect(
      postList
        .locator('[data-testid="test-post-item"]')
        .filter({ hasText: "Third Post (after First)" }),
    ).toBeVisible({ timeout: 5000 });

    // Verify order: First, Third, Second
    await expect(postItems.nth(0)).toContainText("First Post");
    await expect(postItems.nth(1)).toContainText("Third Post (after First)");
    await expect(postItems.nth(2)).toContainText("Second Post");
  });

  test("Ctrl+N creates post below current post", async () => {
    const { page } = userResult;

    const postList = page.locator("#post-list");
    const editor = page.locator('[data-testid="test-editor-content"]');
    const newPostBtn = page.locator("#new-post-btn");

    // Wait for any existing posts
    await expect(
      postList.locator('[data-testid="test-post-wrapper"]').first(),
    ).toBeVisible({ timeout: 10000 });

    // Create a known first post
    await newPostBtn.click();
    await editor.click();
    await page.keyboard.type("CtrlN First");
    await expect(
      postList
        .locator('[data-testid="test-post-item"]')
        .filter({ hasText: "CtrlN First" }),
    ).toBeVisible({ timeout: 5000 });

    // Create a second post (will be after CtrlN First)
    await newPostBtn.click();
    await editor.click();
    await page.keyboard.type("CtrlN Second");
    await expect(
      postList
        .locator('[data-testid="test-post-item"]')
        .filter({ hasText: "CtrlN Second" }),
    ).toBeVisible({ timeout: 5000 });

    // Select CtrlN First
    await postList
      .locator('[data-testid="test-post-item"]')
      .filter({ hasText: "CtrlN First" })
      .click();
    await expect(editor).toContainText("CtrlN First", { timeout: 5000 });

    // Get current count
    const countBefore = await postList
      .locator('[data-testid="test-post-wrapper"]')
      .count();

    // Use Ctrl+N to create new post
    await page.keyboard.press("Control+n");

    // Should have one more post
    await expect(
      postList.locator('[data-testid="test-post-wrapper"]'),
    ).toHaveCount(countBefore + 1, { timeout: 5000 });

    // Name it
    await editor.click();
    await page.keyboard.type("CtrlN Middle");

    // Wait for title
    await expect(
      postList
        .locator('[data-testid="test-post-item"]')
        .filter({ hasText: "CtrlN Middle" }),
    ).toBeVisible({ timeout: 5000 });

    // Find the indices of our posts
    const postItems = postList.locator('[data-testid="test-post-item"]');
    const count = await postItems.count();

    let firstIdx = -1,
      middleIdx = -1,
      secondIdx = -1;
    for (let i = 0; i < count; i++) {
      const text = await postItems.nth(i).textContent();
      if (text?.includes("CtrlN First")) firstIdx = i;
      if (text?.includes("CtrlN Middle")) middleIdx = i;
      if (text?.includes("CtrlN Second")) secondIdx = i;
    }

    // CtrlN Middle should be between CtrlN First and CtrlN Second
    expect(middleIdx).toBe(firstIdx + 1);
    expect(secondIdx).toBe(middleIdx + 1);
  });

  test("new post is created at root when current post is at root", async () => {
    const { page } = userResult;

    const postList = page.locator("#post-list");
    const newPostBtn = page.locator("#new-post-btn");

    // All posts should be at root level (depth 0)
    const wrappers = postList.locator('[data-testid="test-post-wrapper"]');
    const count = await wrappers.count();

    for (let i = 0; i < count; i++) {
      const depth = await wrappers.nth(i).getAttribute("data-depth");
      expect(depth).toBe("0");
    }

    // Create another post
    await newPostBtn.click();

    // Wait for it
    await expect(wrappers).toHaveCount(count + 1, { timeout: 5000 });

    // The new post should also be at depth 0
    const newPostDepth = await wrappers.last().getAttribute("data-depth");
    expect(newPostDepth).toBe("0");
  });
});
