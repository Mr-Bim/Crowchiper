import {
  test,
  expect,
  createUser,
  uniqueTestId,
  CreateUserResult,
} from "../utils/fixtures.ts";
import { getServer, Server } from "../utils/server.ts";
import { BrowserContext, Page } from "@playwright/test";
import path from "path";
import { fileURLToPath } from "url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));

const TEST_IMAGES = [
  path.join(__dirname, "../assets/test-image-1.png"),
  path.join(__dirname, "../assets/test-image-2.png"),
  path.join(__dirname, "../assets/test-image-3.png"),
  path.join(__dirname, "../assets/test-image-4.png"),
];

/**
 * Helper to trigger /Image command and upload files.
 * Returns the UUIDs of the uploaded attachments.
 */
async function uploadViaSlashCommand(
  page: Page,
  files: string[],
): Promise<string[]> {
  const editor = page.locator(".cm-content");
  await editor.click();

  const fileChooserPromise = page.waitForEvent("filechooser");

  await page.keyboard.type("/Image");
  const option = page.locator(".cm-tooltip-autocomplete .cm-completionLabel", {
    hasText: "/Image",
  });
  await expect(option).toBeVisible({ timeout: 5000 });
  await option.click();

  const fileChooser = await fileChooserPromise;

  // Collect all upload responses
  const uuids: string[] = [];
  const responsePromises: Promise<void>[] = [];

  for (let i = 0; i < files.length; i++) {
    responsePromises.push(
      page
        .waitForResponse(
          (response) =>
            response.url().includes("/api/attachments") &&
            response.request().method() === "POST" &&
            response.status() === 201,
          { timeout: 30000 },
        )
        .then(async (response) => {
          const json = (await response.json()) as { uuid: string };
          uuids.push(json.uuid);
        }),
    );
  }

  await fileChooser.setFiles(files);
  await Promise.all(responsePromises);

  return uuids;
}

/**
 * Helper to save the current post using force save button.
 */
async function forceSave(page: Page): Promise<void> {
  const syncIndicator = page.locator('[data-testid="test-sync-indicator"]');
  const forceSaveBtn = page.locator('[data-testid="test-force-save-btn"]');

  const status = await syncIndicator.getAttribute("data-status");

  if (status === "idle" || status === "synced") {
    return;
  }

  if (status === "pending") {
    await forceSaveBtn.click();
  }

  // Whether we clicked or it was already syncing, wait for completion
  await expect(syncIndicator).toHaveAttribute(
    "data-status",
    /^(synced|idle)$/,
    { timeout: 10000 },
  );
}

test.describe("Multi-image upload and removal", () => {
  let context: BrowserContext;
  let userResult: CreateUserResult;
  let baseUrl: string;

  test.beforeAll(async ({ browser }) => {
    const server = await getServer(Server.Default);
    baseUrl = server.baseUrl;
    context = await browser.newContext();
    const username = `multiupload_${uniqueTestId()}`;
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

  test("uploads multiple images into a single gallery", async () => {
    const { page } = userResult;

    // Create a new post
    await page.locator("#new-post-btn").click();
    await expect(page.locator(".cm-content")).toBeVisible({ timeout: 5000 });

    // Upload 3 images at once
    const uuids = await uploadViaSlashCommand(page, TEST_IMAGES.slice(0, 3));
    expect(uuids).toHaveLength(3);

    // Verify all UUIDs are valid
    for (const uuid of uuids) {
      expect(uuid).toMatch(/^[a-f0-9-]{36}$/);
    }

    // All 3 images should appear as thumbnails in the gallery widget
    const galleryImages = page.locator(".cm-gallery-image");
    await expect(galleryImages).toHaveCount(3, { timeout: 10000 });

    // Each uploaded attachment should be accessible via the API
    for (const uuid of uuids) {
      const response = await page.request.get(
        `${baseUrl}/api/attachments/${uuid}/thumbnail/sm`,
      );
      expect(response.status()).toBe(200);
    }
  });

  test("removes individual images from a gallery", async () => {
    const { page } = userResult;

    // Create a new post
    await page.locator("#new-post-btn").click();
    await expect(page.locator(".cm-content")).toBeVisible({ timeout: 5000 });

    // Upload 3 images
    const uuids = await uploadViaSlashCommand(page, TEST_IMAGES.slice(0, 3));
    expect(uuids).toHaveLength(3);

    // Wait for all gallery images to render
    const galleryImages = page.locator(".cm-gallery-image");
    await expect(galleryImages).toHaveCount(3, { timeout: 10000 });

    // Delete the first image by clicking the X button on it
    const firstDeleteBtn = galleryImages
      .first()
      .locator(".cm-gallery-delete-btn");
    await firstDeleteBtn.click();

    // Should now have 2 images
    await expect(galleryImages).toHaveCount(2, { timeout: 5000 });

    // Save to persist the change
    await forceSave(page);

    // Verify the gallery still shows exactly 2 images after save
    await expect(galleryImages).toHaveCount(2, { timeout: 5000 });
  });

  test("deletes entire gallery", async () => {
    const { page } = userResult;

    // Create a new post
    await page.locator("#new-post-btn").click();
    await expect(page.locator(".cm-content")).toBeVisible({ timeout: 5000 });

    // Upload 2 images
    const uuids = await uploadViaSlashCommand(page, TEST_IMAGES.slice(0, 2));
    expect(uuids).toHaveLength(2);

    // Wait for gallery to render
    const galleryContainer = page.locator(".cm-gallery-container");
    await expect(galleryContainer).toBeVisible({ timeout: 10000 });

    // Click the delete gallery button (trash icon in options panel)
    const deleteGalleryBtn = galleryContainer.locator(
      ".cm-gallery-option-btn-danger",
    );
    await deleteGalleryBtn.click();

    // Gallery should be gone
    await expect(galleryContainer).toHaveCount(0, { timeout: 5000 });

    // Save
    await forceSave(page);
  });

  test("adds images to existing gallery via add button", async () => {
    const { page } = userResult;

    // Create a new post
    await page.locator("#new-post-btn").click();
    await expect(page.locator(".cm-content")).toBeVisible({ timeout: 5000 });

    // Upload 1 image to create a gallery
    const initialUuids = await uploadViaSlashCommand(
      page,
      TEST_IMAGES.slice(0, 1),
    );
    expect(initialUuids).toHaveLength(1);

    // Wait for gallery to render
    const galleryContainer = page.locator(".cm-gallery-container");
    await expect(galleryContainer).toBeVisible({ timeout: 10000 });
    const galleryImages = page.locator(".cm-gallery-image");
    await expect(galleryImages).toHaveCount(1, { timeout: 5000 });

    // Click the add image button (+ icon in options panel)
    const addBtn = galleryContainer.locator(
      ".cm-gallery-option-btn:not(.cm-gallery-option-btn-danger)",
    );

    const fileChooserPromise = page.waitForEvent("filechooser");
    await addBtn.click();

    const fileChooser = await fileChooserPromise;

    // Add 2 more images
    const uploadResponsePromise1 = page.waitForResponse(
      (response) =>
        response.url().includes("/api/attachments") &&
        response.request().method() === "POST" &&
        response.status() === 201,
      { timeout: 30000 },
    );
    const uploadResponsePromise2 = page.waitForResponse(
      (response) =>
        response.url().includes("/api/attachments") &&
        response.request().method() === "POST" &&
        response.status() === 201,
      { timeout: 30000 },
    );

    await fileChooser.setFiles(TEST_IMAGES.slice(1, 3));
    await Promise.all([uploadResponsePromise1, uploadResponsePromise2]);

    // Should now have 3 images in the gallery
    await expect(galleryImages).toHaveCount(3, { timeout: 10000 });
  });

  test("saves attachment references to server on force save", async () => {
    const { page } = userResult;

    // Create a new post
    await page.locator("#new-post-btn").click();
    await expect(page.locator(".cm-content")).toBeVisible({ timeout: 5000 });

    // Upload 2 images
    const uuids = await uploadViaSlashCommand(page, TEST_IMAGES.slice(0, 2));
    expect(uuids).toHaveLength(2);

    // Wait for gallery to render
    await expect(page.locator(".cm-gallery-image")).toHaveCount(2, {
      timeout: 10000,
    });

    // Force save to persist
    await forceSave(page);

    // Both attachments should still be accessible (not orphaned/deleted)
    for (const uuid of uuids) {
      const response = await page.request.get(
        `${baseUrl}/api/attachments/${uuid}/thumbnail/sm`,
      );
      expect(response.status()).toBe(200);
    }
  });
});
