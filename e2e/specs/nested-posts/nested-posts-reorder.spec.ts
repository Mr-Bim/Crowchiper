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

test.describe("Nested posts - Reordering", () => {
  test.describe.configure({ mode: "serial" });

  let context: BrowserContext;
  let userResult: CreateUserResult;
  let baseUrl: string;

  test.beforeAll(async ({ browser }) => {
    const server = await getServer(Server.Default);
    baseUrl = server.baseUrl;
    context = await browser.newContext();
    const username = `nested_reorder_${uniqueTestId()}`;
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

  test("can reorder posts within same parent", async () => {
    const { page } = userResult;
    const postList = page.locator("#post-list");

    // Wait for initial post
    await expect(
      postList.locator('[data-testid="test-post-wrapper"]'),
    ).toHaveCount(1, {
      timeout: 10000,
    });

    // Create posts: Post A, Post B, Post C
    await createPostWithTitle(page, "Post A");
    await createPostWithTitle(page, "Post B");
    await createPostWithTitle(page, "Post C");

    await expect(
      postList.locator('[data-testid="test-post-wrapper"]'),
    ).toHaveCount(4, {
      timeout: 5000,
    });

    // Order should be: Post C, Post B, Post A, Untitled (newest first)
    const wrappers = postList.locator('[data-testid="test-post-wrapper"]');
    await expect(
      wrappers.nth(0).locator('[data-testid="test-post-item"]'),
    ).toHaveText("Post C");
    await expect(
      wrappers.nth(1).locator('[data-testid="test-post-item"]'),
    ).toHaveText("Post B");
    await expect(
      wrappers.nth(2).locator('[data-testid="test-post-item"]'),
    ).toHaveText("Post A");

    // Drag Post C below Post B (reorder within siblings)
    await dragPostToPost(page, "Post C", "Post B", {
      centerTarget: false,
      topEdge: false,
    });

    // New order should be: Post B, Post C, Post A, Untitled
    await expect(
      wrappers.nth(0).locator('[data-testid="test-post-item"]'),
    ).toHaveText("Post B");
    await expect(
      wrappers.nth(1).locator('[data-testid="test-post-item"]'),
    ).toHaveText("Post C");
  });

  test("reorder within nested children works correctly", async () => {
    const { page } = userResult;

    // Create a parent folder
    await createPostWithTitle(page, "Reorder Parent");

    // Create two children and drag them under the parent
    await createPostWithTitle(page, "Child 1");
    await createPostWithTitle(page, "Child 2");

    // Drag Child 1 under Reorder Parent
    await dragPostToPost(page, "Child 1", "Reorder Parent", {
      centerTarget: true,
    });

    // Drag Child 2 under Reorder Parent
    await dragPostToPost(page, "Child 2", "Reorder Parent", {
      centerTarget: true,
    });

    // Both children should be at depth 1
    const child1 = getPostByTitle(page, "Child 1");
    const child2 = getPostByTitle(page, "Child 2");
    await expect(child1).toHaveAttribute("data-depth", "1");
    await expect(child2).toHaveAttribute("data-depth", "1");

    // Verify they have the same parent
    const parentPost = getPostByTitle(page, "Reorder Parent");
    const parentUuid = await parentPost.getAttribute("data-post-uuid");
    await expect(child1).toHaveAttribute("data-parent-id", parentUuid!);
    await expect(child2).toHaveAttribute("data-parent-id", parentUuid!);

    // Now reorder Child 1 above Child 2 (within the same parent)
    await dragPostToPost(page, "Child 1", "Child 2", {
      centerTarget: false,
      topEdge: true,
    });

    // Verify both are still children of the parent
    const child1Final = getPostByTitle(page, "Child 1");
    const child2Final = getPostByTitle(page, "Child 2");
    await expect(child1Final).toHaveAttribute("data-parent-id", parentUuid!);
    await expect(child2Final).toHaveAttribute("data-parent-id", parentUuid!);
  });
});
