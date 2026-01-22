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

test.describe("Nested posts - Delete operations", () => {
  test.describe.configure({ mode: "serial" });

  let context: BrowserContext;
  let userResult: CreateUserResult;
  let baseUrl: string;

  test.beforeAll(async ({ browser }) => {
    const server = await getServer(Server.Default);
    baseUrl = server.baseUrl;
    context = await browser.newContext();
    const username = `nested_delete_${uniqueTestId()}`;
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

  test("can delete a single nested post", async () => {
    const { page } = userResult;
    const postList = page.locator("#post-list");

    // Wait for initial post
    await expect(postList.locator('[data-testid="post-wrapper"]')).toHaveCount(
      1,
      {
        timeout: 10000,
      },
    );

    // Create parent and child
    await createPostWithTitle(page, "Delete Parent");
    await createPostWithTitle(page, "Delete Child");

    const initialCount = await postList
      .locator('[data-testid="post-wrapper"]')
      .count();

    // Drag child under parent
    await dragPostToPost(page, "Delete Child", "Delete Parent", {
      centerTarget: true,
    });

    // Select the child post by clicking on it
    const childUpdated = getPostByTitle(page, "Delete Child");
    await childUpdated.locator('[data-testid="post-item"]').click();
    await page.waitForTimeout(300);

    // Set up dialog handler for confirmation (use once to avoid affecting other tests)
    page.once("dialog", (dialog) => dialog.accept());

    // Delete the child
    const deleteBtn = page.locator("#delete-btn");
    await deleteBtn.click();

    // Wait for deletion
    await page.waitForTimeout(500);

    // Child should be gone
    await expect(getPostByTitle(page, "Delete Child")).not.toBeVisible();

    // Parent should no longer have expand button (no children)
    const parentUpdated = getPostByTitle(page, "Delete Parent");
    const expandBtn = parentUpdated.locator('[data-testid="post-expand-btn"]');
    await expect(expandBtn).toHaveCount(0);

    // Total count should be reduced by 1
    await expect(postList.locator('[data-testid="post-wrapper"]')).toHaveCount(
      initialCount - 1,
    );
  });

  test("delete parent with children shows cascade confirmation", async () => {
    const { page } = userResult;

    // Create parent
    await createPostWithTitle(page, "Cascade Parent");

    // Create multiple children
    await createPostWithTitle(page, "Cascade Child 1");
    await createPostWithTitle(page, "Cascade Child 2");

    // Drag both children under parent
    await dragPostToPost(page, "Cascade Child 1", "Cascade Parent", {
      centerTarget: true,
    });
    await dragPostToPost(page, "Cascade Child 2", "Cascade Parent", {
      centerTarget: true,
    });

    // Verify children are nested
    const child1 = getPostByTitle(page, "Cascade Child 1");
    const child2 = getPostByTitle(page, "Cascade Child 2");
    await expect(child1).toHaveAttribute("data-depth", "1");
    await expect(child2).toHaveAttribute("data-depth", "1");

    // Select the parent post
    const cascadeParent = getPostByTitle(page, "Cascade Parent");
    await cascadeParent.locator('[data-testid="post-item"]').click();
    await page.waitForTimeout(300);

    // Track dialog message (use once to avoid affecting other tests)
    let dialogMessage = "";
    page.once("dialog", async (dialog) => {
      dialogMessage = dialog.message();
      await dialog.accept();
    });

    // Delete the parent
    const deleteBtn = page.locator("#delete-btn");
    await deleteBtn.click();

    // Wait for deletion
    await page.waitForTimeout(500);

    // Verify the confirmation message mentioned nested posts
    expect(dialogMessage).toContain("nested posts");
    expect(dialogMessage).toContain("deleted");

    // Parent and all children should be gone
    await expect(getPostByTitle(page, "Cascade Parent")).not.toBeVisible();
    await expect(getPostByTitle(page, "Cascade Child 1")).not.toBeVisible();
    await expect(getPostByTitle(page, "Cascade Child 2")).not.toBeVisible();
  });

  test("canceling cascade delete keeps posts intact", async () => {
    const { page } = userResult;
    const postList = page.locator("#post-list");

    // Create parent and child
    await createPostWithTitle(page, "Cancel Parent");
    await createPostWithTitle(page, "Cancel Child");

    // Drag child under parent
    await dragPostToPost(page, "Cancel Child", "Cancel Parent", {
      centerTarget: true,
    });

    const countBefore = await postList
      .locator('[data-testid="post-wrapper"]')
      .count();

    // Select parent
    const cancelParent = getPostByTitle(page, "Cancel Parent");
    await cancelParent.locator('[data-testid="post-item"]').click();
    await page.waitForTimeout(300);

    // Set up dialog to dismiss (cancel) - use once to avoid affecting other tests
    page.once("dialog", (dialog) => dialog.dismiss());

    // Try to delete
    const deleteBtn = page.locator("#delete-btn");
    await deleteBtn.click();
    await page.waitForTimeout(500);

    // Both posts should still exist
    await expect(getPostByTitle(page, "Cancel Parent")).toBeVisible();
    await expect(getPostByTitle(page, "Cancel Child")).toBeVisible();

    // Count should be same
    await expect(postList.locator('[data-testid="post-wrapper"]')).toHaveCount(
      countBefore,
    );
  });
});
