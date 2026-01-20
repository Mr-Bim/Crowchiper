import { Page } from "@playwright/test";
import { expect } from "../../utils/fixtures.ts";

/**
 * Helper to create a post with a specific title.
 */
export async function createPostWithTitle(
  page: Page,
  title: string,
): Promise<void> {
  const newPostBtn = page.locator("#new-post-btn");
  await newPostBtn.click();

  // Wait for new post to be created and editor to be ready
  const editor = page.locator("#editor .cm-content");
  await editor.click();
  await page.keyboard.type(title);

  // Wait for title to appear in sidebar
  const postList = page.locator("#post-list");
  await expect(
    postList.locator(".post-item").filter({ hasText: title }),
  ).toBeVisible({ timeout: 5000 });
}

/**
 * Helper to save the current post.
 */
export async function savePost(page: Page): Promise<void> {
  const saveBtn = page.locator("#save-btn");
  // Wait for dirty state if there are unsaved changes
  const isDirty = await saveBtn.getAttribute("data-dirty");
  if (isDirty === "true") {
    await saveBtn.click();
    await expect(saveBtn).toHaveAttribute("data-dirty", "false", {
      timeout: 5000,
    });
  }
}

/**
 * Helper to get a post wrapper by title.
 */
export function getPostByTitle(page: Page, title: string) {
  const postList = page.locator("#post-list");
  return postList.locator(".post-wrapper").filter({
    has: page.locator(".post-item", { hasText: title }),
  });
}

/**
 * Helper to reload and unlock the page.
 */
export async function reloadAndUnlock(page: Page): Promise<void> {
  await page.reload();
  const unlockBtn = page.locator("#unlock-btn");
  await unlockBtn.waitFor({ state: "visible" });
  await unlockBtn.click();
  const unlockOverlay = page.locator("#unlock-overlay");
  await unlockOverlay.waitFor({ state: "hidden", timeout: 10000 });
}

/**
 * Helper to drag one post onto another.
 * @param centerTarget If true, drags to center (reparent). If false, drags to edge (reorder).
 */
export async function dragPostToPost(
  page: Page,
  sourceTitle: string,
  targetTitle: string,
  options: { centerTarget?: boolean; topEdge?: boolean } = {},
): Promise<void> {
  const { centerTarget = true, topEdge = false } = options;

  const sourcePost = getPostByTitle(page, sourceTitle);
  const targetPost = getPostByTitle(page, targetTitle);

  const sourceBox = await sourcePost.boundingBox();
  const targetBox = await targetPost.boundingBox();

  if (!sourceBox || !targetBox) {
    throw new Error("Could not get bounding boxes for drag and drop");
  }

  let targetY: number;
  if (centerTarget) {
    targetY = targetBox.y + targetBox.height / 2; // Center = reparent
  } else if (topEdge) {
    targetY = targetBox.y + targetBox.height * 0.15; // Top edge = reorder above
  } else {
    targetY = targetBox.y + targetBox.height * 0.85; // Bottom edge = reorder below
  }

  await page.mouse.move(
    sourceBox.x + sourceBox.width / 2,
    sourceBox.y + sourceBox.height / 2,
  );
  await page.mouse.down();
  await page.mouse.move(targetBox.x + targetBox.width / 2, targetY, {
    steps: 10,
  });
  await page.mouse.up();
  await page.waitForTimeout(500);
}
