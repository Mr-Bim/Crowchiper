import {
  test,
  expect,
  APP_PATH,
  addVirtualAuthenticator,
  generateTestPrfOutput,
  injectTestPrfOutput,
  injectTestUsername,
} from "./fixtures.ts";

test.describe("Encryption setup flow", () => {
  test("registers user, tests PRF support, enables encryption", async ({
    context,
    baseUrl,
  }) => {
    const page = await context.newPage();
    const client = await page.context().newCDPSession(page);
    await addVirtualAuthenticator(client);

    const username = "prfuser";

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
  }) => {
    const page = await context.newPage();
    const client = await page.context().newCDPSession(page);
    // No PRF support and no injected PRF output
    await addVirtualAuthenticator(client, { hasPrf: false });

    const username = "noprfuser";
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

  test("user can unlock with PRF and view posts", async ({
    context,
    baseUrl,
  }) => {
    const page = await context.newPage();
    const client = await page.context().newCDPSession(page);
    await addVirtualAuthenticator(client);

    const username = "unlockuser";

    // Inject test PRF output and username
    const testPrfOutput = generateTestPrfOutput();
    await injectTestPrfOutput(page, testPrfOutput);
    await injectTestUsername(page, username);

    // Register and enable encryption
    await page.goto(`${baseUrl}/login/register.html`);
    await expect(page.locator("#register-button")).toBeEnabled({
      timeout: 5000,
    });

    await page.fill("#username", username);
    await page.click("#register-button");

    await expect(page).toHaveTitle("Setup Encryption - Crowchiper", {
      timeout: 10000,
    });

    const testPrfBtn = page.locator("#test-prf-btn");
    await expect(testPrfBtn).toBeEnabled({ timeout: 5000 });
    await testPrfBtn.click();

    const enableEncryptionBtn = page.locator("#enable-encryption-btn");
    await expect(enableEncryptionBtn).toBeVisible({ timeout: 10000 });
    await enableEncryptionBtn.click();

    // Should redirect to app with unlock overlay
    await expect(page).toHaveURL(new RegExp(`${APP_PATH}`), { timeout: 10000 });

    const unlockOverlay = page.locator("#unlock-overlay");
    await expect(unlockOverlay).toBeVisible({ timeout: 5000 });

    // Click unlock button
    const unlockBtn = page.locator("#unlock-btn");
    await expect(unlockBtn).toBeEnabled({ timeout: 5000 });
    await unlockBtn.click();

    // Unlock overlay should disappear
    await expect(unlockOverlay).toBeHidden({ timeout: 10000 });

    // The editor should be visible (app is functional)
    const editor = page.locator("#editor");
    await expect(editor).toBeVisible({ timeout: 5000 });

    await page.close();
  });

  test("can create and save encrypted post", async ({ context, baseUrl }) => {
    const page = await context.newPage();
    const client = await page.context().newCDPSession(page);
    await addVirtualAuthenticator(client);

    const username = "encryptedpostuser";

    // Inject test PRF output and username
    const testPrfOutput = generateTestPrfOutput();
    await injectTestPrfOutput(page, testPrfOutput);
    await injectTestUsername(page, username);

    // Register and enable encryption
    await page.goto(`${baseUrl}/login/register.html`);
    await expect(page.locator("#register-button")).toBeEnabled({
      timeout: 5000,
    });

    await page.fill("#username", username);
    await page.click("#register-button");

    await expect(page).toHaveTitle("Setup Encryption - Crowchiper", {
      timeout: 10000,
    });

    const testPrfBtn = page.locator("#test-prf-btn");
    await expect(testPrfBtn).toBeEnabled({ timeout: 5000 });
    await testPrfBtn.click();

    const enableEncryptionBtn = page.locator("#enable-encryption-btn");
    await expect(enableEncryptionBtn).toBeVisible({ timeout: 10000 });
    await enableEncryptionBtn.click();

    // Unlock
    await expect(page).toHaveURL(new RegExp(`${APP_PATH}`), { timeout: 10000 });

    const unlockBtn = page.locator("#unlock-btn");
    await expect(unlockBtn).toBeEnabled({ timeout: 5000 });
    await unlockBtn.click();

    const unlockOverlay = page.locator("#unlock-overlay");
    await expect(unlockOverlay).toBeHidden({ timeout: 10000 });

    // Create a new post
    const newPostBtn = page.locator("#new-post-btn");
    await newPostBtn.click();

    // Type content in the editor
    const editorContent = page.locator(".cm-content");
    await editorContent.click();
    await page.keyboard.type("# My Secret Note\n\nThis is encrypted content.");

    // Save the post
    const saveBtn = page.locator("#save-btn");
    await expect(saveBtn).toHaveAttribute("data-dirty", "true", {
      timeout: 5000,
    });
    await saveBtn.click();

    // Wait for save to complete
    await expect(saveBtn).toHaveAttribute("data-dirty", "false", {
      timeout: 5000,
    });

    // Post should appear in the list
    const postList = page.locator("#post-list");
    await expect(postList.locator(".post-wrapper")).toHaveCount(2, {
      timeout: 5000,
    }); // 1 new post + 1 default "Untitled"

    await page.close();
  });
});
