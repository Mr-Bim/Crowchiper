import {
  test,
  expect,
  createUser,
  uniqueTestId,
  CreateUserResult,
} from "../../utils/fixtures.ts";
import { getServer, Server } from "../../utils/server.ts";
import { BrowserContext } from "@playwright/test";
import { createPostWithTitle } from "./nested-posts-helpers.ts";

test.describe("Nested posts - Basic operations", () => {
  let context: BrowserContext;
  let userResult: CreateUserResult;
  let baseUrl: string;

  test.beforeAll(async ({ browser }) => {
    const server = await getServer(Server.Default);
    baseUrl = server.baseUrl;
    context = await browser.newContext();
    const username = `nested_basic_${uniqueTestId()}`;
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

  test("can create multiple posts and verify tree structure", async () => {
    const { page } = userResult;
    const postList = page.locator("#post-list");

    // Initial state - should have one auto-created post
    await expect(postList.locator('[data-testid="post-wrapper"]')).toHaveCount(
      1,
      {
        timeout: 10000,
      },
    );

    // Create first named post
    await createPostWithTitle(page, "Parent Post");
    await expect(postList.locator('[data-testid="post-wrapper"]')).toHaveCount(
      2,
      {
        timeout: 5000,
      },
    );

    // Create second post
    await createPostWithTitle(page, "Child Post");
    await expect(postList.locator('[data-testid="post-wrapper"]')).toHaveCount(
      3,
      {
        timeout: 5000,
      },
    );

    // Verify all posts are at depth 0 (root level)
    const wrappers = postList.locator('[data-testid="post-wrapper"]');
    const count = await wrappers.count();
    for (let i = 0; i < count; i++) {
      const depth = await wrappers.nth(i).getAttribute("data-depth");
      expect(depth).toBe("0");
    }
  });
});
