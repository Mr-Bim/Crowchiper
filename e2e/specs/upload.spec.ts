import {
  test,
  expect,
  createUser,
  uniqueTestId,
  CreateUserResult,
} from "../utils/fixtures.ts";
import { getServer, Server } from "../utils/server.ts";
import { BrowserContext } from "@playwright/test";
import path from "path";
import { fileURLToPath } from "url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));

test.describe("Image upload", () => {
  let context: BrowserContext;
  let userResult: CreateUserResult;
  let baseUrl: string;

  test.beforeAll(async ({ browser }) => {
    const server = await getServer(Server.Default);
    baseUrl = server.baseUrl;
    context = await browser.newContext();
    const username = `upload_${uniqueTestId()}`;
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

  test("uploads encrypted image via /Image command", async () => {
    const { page } = userResult;

    // Create a new post
    const newPostBtn = page.locator("#new-post-btn");
    await newPostBtn.click();

    // Wait for editor to be ready
    const editorContent = page.locator(".cm-content");
    await expect(editorContent).toBeVisible({ timeout: 5000 });
    await editorContent.click();

    // Set up file chooser listener before triggering the command
    const fileChooserPromise = page.waitForEvent("filechooser");

    // Type /Image to trigger the slash command
    await page.keyboard.type("/Image");

    // Wait for autocomplete and select the Image option
    const autocompleteOption = page.locator(
      ".cm-tooltip-autocomplete .cm-completionLabel",
      { hasText: "/Image" },
    );
    await expect(autocompleteOption).toBeVisible({ timeout: 5000 });
    await autocompleteOption.click();

    // Handle the file chooser and wait for the attachment upload API call
    const fileChooser = await fileChooserPromise;
    const testImagePath = path.join(
      __dirname,
      "../assets/schachtzabel-0025.webp",
    );

    // Set up promise to wait for the attachment upload response
    const uploadResponsePromise = page.waitForResponse(
      (response) =>
        response.url().includes("/api/attachments") &&
        response.request().method() === "POST" &&
        response.status() === 201,
    );

    await fileChooser.setFiles(testImagePath);

    // Wait for the upload to complete
    const uploadResponse = await uploadResponsePromise;
    const uploadResult = (await uploadResponse.json()) as { uuid: string };
    const uuid = uploadResult.uuid;

    // Verify we got a valid UUID
    expect(uuid).toMatch(/^[a-f0-9-]{36}$/);

    // Fetch the attachment and verify it has encryption IV header
    const response = await page.request.get(
      `${baseUrl}/api/attachments/${uuid}/thumbnail/sm`,
    );
    expect(response.status()).toBe(200);

    // The presence of X-Encryption-IV header indicates the image is encrypted
    const ivHeader = response.headers()["x-encryption-iv"];
    expect(ivHeader).toBeTruthy();
    expect(ivHeader.length).toBeGreaterThan(0);
  });
});
