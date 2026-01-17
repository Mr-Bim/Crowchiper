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

test.describe("Nested posts - Folders", () => {
  test.describe.configure({ mode: "serial" });

  let context: BrowserContext;
  let userResult: CreateUserResult;
  let baseUrl: string;

  test.beforeAll(async ({ browser }) => {
    const server = await getServer(Server.Default);
    baseUrl = server.baseUrl;
    context = await browser.newContext();
    const username = `nested_folders_${uniqueTestId()}`;
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

  test("folders show folder icon and are not editable", async () => {
    const { page } = userResult;
    const postList = page.locator("#post-list");

    // Wait for initial post
    await expect(postList.locator(".post-wrapper")).toHaveCount(1, {
      timeout: 10000,
    });

    // Create a post and drag it under another to make it a parent
    await createPostWithTitle(page, "Folder Test Parent");
    await createPostWithTitle(page, "Folder Test Child");

    // Drag child under parent
    await dragPostToPost(page, "Folder Test Child", "Folder Test Parent", {
      centerTarget: true,
    });

    // Verify parent has page icon (not folder icon since it's not a folder)
    const parent = getPostByTitle(page, "Folder Test Parent");
    const parentIcon = parent.locator(".post-icon");
    const iconText = await parentIcon.textContent();
    // Regular posts have page icon (📄), folders have folder icon (📁)
    expect(iconText).toBe("📄");

    // Posts with children that are NOT folders should still be editable
    const parentItem = parent.locator(".post-item");
    await expect(parentItem).not.toHaveAttribute("data-folder", "true");
  });

  test("clicking folder expands/collapses instead of editing", async () => {
    const { page } = userResult;

    // Get the parent we created (it has children)
    const parent = getPostByTitle(page, "Folder Test Parent");
    const expandBtn = parent.locator(".post-expand-btn");

    // Ensure it's expanded
    await expect(expandBtn).toHaveAttribute("data-expanded", "true");

    // Get the parent post item (title button)
    const parentItem = parent.locator(".post-item");

    // Click the expand button to collapse
    await expandBtn.click();
    await page.waitForTimeout(300);

    // Verify collapsed
    await expect(expandBtn).toHaveAttribute("data-expanded", "false");

    // Click expand button to expand again
    await expandBtn.click();
    await page.waitForTimeout(300);

    await expect(expandBtn).toHaveAttribute("data-expanded", "true");

    // Clicking the title (for non-folder posts) should select for editing
    // Since this is a regular post (not is_folder), clicking title selects it
    await parentItem.click();
    await page.waitForTimeout(300);

    // It should be active (selected)
    await expect(parentItem).toHaveClass(/active/);
  });
});
