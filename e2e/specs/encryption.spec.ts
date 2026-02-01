import {
  test,
  expect,
  APP_PATH,
  addVirtualAuthenticator,
  generateTestPrfOutput,
  injectTestPrfOutput,
  injectTestUsername,
  createUser,
  uniqueTestId,
  CreateUserResult,
} from "../utils/fixtures.ts";
import { getServer, Server } from "../utils/server.ts";
import { BrowserContext } from "@playwright/test";

test.describe("Encryption setup flow", () => {
  test("registers user, tests PRF support, enables encryption", async ({
    context,
    baseUrl,
    testId,
  }) => {
    const page = await context.newPage();
    const client = await page.context().newCDPSession(page);
    await addVirtualAuthenticator(client);

    const username = `prf_${testId}`;

    // Inject test PRF output and username (Chrome's virtual authenticator limitations)
    const testPrfOutput = generateTestPrfOutput();
    await injectTestPrfOutput(page, testPrfOutput);
    await injectTestUsername(page, username);

    // Register a new user
    await page.goto(`${baseUrl}/login/register.html`);
    await expect(page.locator("#register-button")).toBeEnabled({
      timeout: 5000,
    });

    await page.fill("#username", username);
    await page.click("#register-button");

    // Wait for redirect to setup-encryption page
    await expect(page).toHaveTitle("Setup Encryption - Crowchiper", {
      timeout: 10000,
    });

    // Click "Test Passkey" to test PRF support
    const testPrfBtn = page.locator("#test-prf-btn");
    await expect(testPrfBtn).toBeEnabled({ timeout: 5000 });
    await testPrfBtn.click();

    // Since we have PRF injected, we should see the "PRF supported" step
    const enableEncryptionBtn = page.locator("#enable-encryption-btn");
    await expect(enableEncryptionBtn).toBeVisible({ timeout: 10000 });

    // Enable encryption
    await enableEncryptionBtn.click();

    // Should redirect to app
    await expect(page).toHaveURL(new RegExp(`${APP_PATH}`), { timeout: 10000 });

    // The unlock overlay should be visible (encryption is enabled)
    const unlockOverlay = page.locator("#unlock-overlay");
    await expect(unlockOverlay).toBeVisible({ timeout: 5000 });

    await page.close();
  });

  test("registers user without PRF, skips encryption", async ({
    context,
    baseUrl,
    testId,
  }) => {
    const page = await context.newPage();
    const client = await page.context().newCDPSession(page);
    // No PRF support and no injected PRF output
    await addVirtualAuthenticator(client, { hasPrf: false });

    const username = `noprf_${testId}`;
    await injectTestUsername(page, username);

    // Register a new user
    await page.goto(`${baseUrl}/login/register.html`);
    await expect(page.locator("#register-button")).toBeEnabled({
      timeout: 5000,
    });

    await page.fill("#username", username);
    await page.click("#register-button");

    // Wait for redirect to setup-encryption page
    await expect(page).toHaveTitle("Setup Encryption - Crowchiper", {
      timeout: 10000,
    });

    // Click "Test Passkey" to test PRF support
    const testPrfBtn = page.locator("#test-prf-btn");
    await expect(testPrfBtn).toBeEnabled({ timeout: 5000 });
    await testPrfBtn.click();

    // Since PRF is not supported, we should see the "no PRF" step
    const continueLink = page.locator("#continue-link");
    await expect(continueLink).toBeVisible({ timeout: 10000 });

    // Click continue to go to app
    await continueLink.click();

    // Should redirect to app
    await expect(page).toHaveURL(new RegExp(`${APP_PATH}`), { timeout: 10000 });

    // The unlock overlay should NOT be visible (no encryption)
    const unlockOverlay = page.locator("#unlock-overlay");
    await expect(unlockOverlay).toBeHidden({ timeout: 5000 });

    await page.close();
  });
});

test.describe("Encryption usage", () => {
  let context: BrowserContext;
  let userResult: CreateUserResult;

  test.beforeAll(async ({ browser }) => {
    const { baseUrl } = await getServer(Server.Default);
    context = await browser.newContext();
    const username = `encuser_${uniqueTestId()}`;
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

  test("editor is visible after unlock", async () => {
    const { page } = userResult;

    // The editor should be visible (app is functional after createUser unlocks)
    const editor = page.locator("#editor");
    await expect(editor).toBeVisible({ timeout: 5000 });
  });

  test("can create and save encrypted post", async () => {
    const { page } = userResult;

    // Create a new post
    const newPostBtn = page.locator("#new-post-btn");
    await newPostBtn.click();

    // Type content in the editor
    const editorContent = page.locator(".cm-content");
    await editorContent.click();
    await page.keyboard.type("# My Secret Note\n\nThis is encrypted content.");

    // Save the post using force save button
    const syncIndicator = page.locator('[data-testid="test-sync-indicator"]');
    const forceSaveBtn = page.locator('[data-testid="test-force-save-btn"]');

    // Should show pending after typing
    await expect(syncIndicator).toHaveAttribute("data-status", "pending", {
      timeout: 5000,
    });

    // Click force save
    await forceSaveBtn.click();

    // Wait for save to complete (synced then idle)
    await expect(syncIndicator).toHaveAttribute("data-status", "synced", {
      timeout: 5000,
    });

    // Post should appear in the list
    const postList = page.locator("#post-list");
    await expect(
      postList.locator('[data-testid="test-post-wrapper"]'),
    ).toHaveCount(2, {
      timeout: 5000,
    }); // 1 new post + 1 default "Untitled"
  });
});
