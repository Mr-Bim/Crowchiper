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

test.describe("Nested posts - Drag and drop reparenting", () => {
  test.describe.configure({ mode: "serial" });

  let context: BrowserContext;
  let userResult: CreateUserResult;
  let baseUrl: string;

  test.beforeAll(async ({ browser }) => {
    const server = await getServer(Server.Default);
    baseUrl = server.baseUrl;
    context = await browser.newContext();
    const username = `nested_dnd_${uniqueTestId()}`;
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

  test("can drag a post to become a child of another post", async () => {
    const { page } = userResult;
    const postList = page.locator("#post-list");

    // Wait for initial auto-created post
    await expect(postList.locator(".post-wrapper")).toHaveCount(1, {
      timeout: 10000,
    });

    // Create parent post
    await createPostWithTitle(page, "DnD Parent");
    await expect(postList.locator(".post-wrapper")).toHaveCount(2, {
      timeout: 5000,
    });

    // Create child post
    await createPostWithTitle(page, "DnD Child");
    await expect(postList.locator(".post-wrapper")).toHaveCount(3, {
      timeout: 5000,
    });

    // Save the current post to ensure changes are persisted
    await savePost(page);

    // Get post elements - newest posts are at the top
    // Order should be: DnD Child, DnD Parent, Untitled
    const childPost = getPostByTitle(page, "DnD Child");
    const parentPost = getPostByTitle(page, "DnD Parent");

    // Verify initial state - both at depth 0
    await expect(childPost).toHaveAttribute("data-depth", "0");
    await expect(parentPost).toHaveAttribute("data-depth", "0");

    // Drag DnD Child to center of DnD Parent (reparenting)
    await dragPostToPost(page, "DnD Child", "DnD Parent", {
      centerTarget: true,
    });

    // Parent should now have the expand button (has_children = true)
    const parentExpandBtn = parentPost.locator(".post-expand-btn");
    await expect(parentExpandBtn).toBeVisible({ timeout: 5000 });

    // Parent should be expanded by default after reparenting
    await expect(parentExpandBtn).toHaveAttribute("data-expanded", "true");

    // Child should now be at depth 1
    const childPostUpdated = getPostByTitle(page, "DnD Child");
    await expect(childPostUpdated).toHaveAttribute("data-depth", "1");

    // Child should have the parent's UUID as parent-id
    const parentUuid = await parentPost.getAttribute("data-uuid");
    await expect(childPostUpdated).toHaveAttribute(
      "data-parent-id",
      parentUuid!,
    );
  });

  test("reparenting persists after page reload", async () => {
    const { page } = userResult;
    const postList = page.locator("#post-list");

    // Save and reload
    await savePost(page);
    await reloadAndUnlock(page);

    // Wait for posts to load
    await expect(postList.locator(".post-wrapper")).toHaveCount(3, {
      timeout: 10000,
    });

    // Verify DnD Child is still nested under DnD Parent
    const childPost = getPostByTitle(page, "DnD Child");
    await expect(childPost).toHaveAttribute("data-depth", "1");

    // Parent should still have expand button
    const parentPost = getPostByTitle(page, "DnD Parent");
    const parentExpandBtn = parentPost.locator(".post-expand-btn");
    await expect(parentExpandBtn).toBeVisible();
  });
});
