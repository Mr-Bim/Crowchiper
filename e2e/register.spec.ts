import { test, expect, addVirtualAuthenticator } from "./fixtures.ts";

test.describe("Register page", () => {
  test("page loads with correct title", async ({ page, baseUrl }) => {
    await page.goto(`${baseUrl}/login/register.html`);

    await expect(page).toHaveTitle("Register - Crowchiper");
  });

  test("button enabled after JS loads", async ({ page, baseUrl }) => {
    await page.goto(`${baseUrl}/login/register.html`);

    const button = page.locator("#register-button");
    await expect(button).toBeEnabled({ timeout: 5000 });
  });

  test("authenticator type hidden on non-Android", async ({
    page,
    baseUrl,
  }) => {
    await page.goto(`${baseUrl}/login/register.html`);

    // Wait for JS to load
    await expect(page.locator("#register-button")).toBeEnabled({
      timeout: 5000,
    });

    // Verify fieldset is hidden on non-Android (Chrome headless is not Android)
    const fieldset = page.locator("#auth-type-fieldset");
    await expect(fieldset).toBeHidden();
  });

  test("authenticator type shown on Android", async ({ context, baseUrl }) => {
    // Create a new page with Android user agent
    const page = await context.newPage();

    // Set Android user agent via CDP
    const client = await page.context().newCDPSession(page);
    await client.send("Emulation.setUserAgentOverride", {
      userAgent:
        "Mozilla/5.0 (Linux; Android 14) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Mobile Safari/537.36",
    });

    // Enable WebAuthn for this page too
    await addVirtualAuthenticator(client);

    await page.goto(`${baseUrl}/login/register.html`);

    // Wait for JS to load
    await expect(page.locator("#register-button")).toBeEnabled({
      timeout: 5000,
    });

    // Verify fieldset is visible on Android
    const fieldset = page.locator("#auth-type-fieldset");
    await expect(fieldset).toBeVisible();

    // Verify passkey is selected by default
    const passkeyRadio = page.locator(
      'input[name="auth-type"][value="passkey"]',
    );
    await expect(passkeyRadio).toBeChecked();

    await page.close();
  });

  test("login link navigates to login page", async ({ page, baseUrl }) => {
    await page.goto(`${baseUrl}/login/register.html`);

    await page.click('a[href*="index"]');

    await expect(page).toHaveTitle("Crowchiper", { timeout: 5000 });
  });

  test("claim username success", async ({ page, baseUrl }) => {
    await page.goto(`${baseUrl}/login/register.html`);

    // Wait for JS to load
    await expect(page.locator("#register-button")).toBeEnabled({
      timeout: 5000,
    });

    // Type username and click register
    await page.fill("#username", "testuser");
    await page.click("#register-button");

    // Wait for redirect to setup-encryption page (user is now logged in)
    // Virtual authenticator completes registration automatically
    await expect(page).toHaveTitle("Setup Encryption - Crowchiper", {
      timeout: 10000,
    });
  });

  test("claim duplicate username fails", async ({ context, baseUrl }) => {
    // First page - register first user
    const page1 = await context.newPage();
    const client1 = await page1.context().newCDPSession(page1);
    await addVirtualAuthenticator(client1);

    await page1.goto(`${baseUrl}/login/register.html`);
    await expect(page1.locator("#register-button")).toBeEnabled({
      timeout: 5000,
    });

    await page1.fill("#username", "duplicateuser");
    await page1.click("#register-button");

    // Wait for registration to complete
    await expect(page1).toHaveTitle("Setup Encryption - Crowchiper", {
      timeout: 10000,
    });

    // Second page - try to claim same username
    const page2 = await context.newPage();
    const client2 = await page2.context().newCDPSession(page2);

    // Clear cookies for page2 so it's a fresh session
    await client2.send("Network.clearBrowserCookies");

    await addVirtualAuthenticator(client2);

    await page2.goto(`${baseUrl}/login/register.html`);
    await expect(page2.locator("#register-button")).toBeEnabled({
      timeout: 5000,
    });

    await page2.fill("#username", "duplicateuser");
    await page2.click("#register-button");

    // Wait for error message
    const errorMessage = page2.locator("#error-message");
    await expect(errorMessage).toContainText("already taken", {
      timeout: 5000,
    });

    await page1.close();
    await page2.close();
  });

  test("multiple users can register", async ({ context, baseUrl }) => {
    // First user
    const page1 = await context.newPage();
    const client1 = await page1.context().newCDPSession(page1);
    await addVirtualAuthenticator(client1);

    await page1.goto(`${baseUrl}/login/register.html`);
    await expect(page1.locator("#register-button")).toBeEnabled({
      timeout: 5000,
    });

    await page1.fill("#username", "firstuser");
    await page1.click("#register-button");
    await expect(page1).toHaveTitle("Setup Encryption - Crowchiper", {
      timeout: 10000,
    });

    // Second user in new page
    const page2 = await context.newPage();
    const client2 = await page2.context().newCDPSession(page2);
    await addVirtualAuthenticator(client2);

    await page2.goto(`${baseUrl}/login/register.html`);
    await expect(page2.locator("#register-button")).toBeEnabled({
      timeout: 5000,
    });

    await page2.fill("#username", "seconduser");
    await page2.click("#register-button");
    await expect(page2).toHaveTitle("Setup Encryption - Crowchiper", {
      timeout: 10000,
    });

    await page1.close();
    await page2.close();
  });
});

test.describe("Register with signup disabled", () => {
  test("register page redirects to login when signup disabled", async ({
    page,
    serverWithOptions,
  }) => {
    const { baseUrl } = await serverWithOptions({ noSignup: true });
    await page.goto(`${baseUrl}/login/register.html`);

    // Should redirect to login page
    await expect(page).toHaveTitle("Crowchiper", { timeout: 5000 });

    // Verify we're on the login page
    await expect(page.locator("#login-button")).toBeVisible();
  });

  test("register link hidden when signup disabled", async ({
    page,
    serverWithOptions,
  }) => {
    const { baseUrl } = await serverWithOptions({ noSignup: true });
    await page.goto(`${baseUrl}/login/index.html`);

    // Wait for JS to load
    await expect(page.locator("#passkey-button")).toBeEnabled({
      timeout: 5000,
    });

    // Register link should be hidden
    const registerLink = page.locator("#register-link");
    await expect(registerLink).toBeHidden();
  });

  test("register link visible when signup enabled", async ({
    page,
    baseUrl,
  }) => {
    await page.goto(`${baseUrl}/login/index.html`);

    // Wait for JS to load
    await expect(page.locator("#passkey-button")).toBeEnabled({
      timeout: 5000,
    });

    // Wait for register link to become visible (config fetched)
    const registerLink = page.locator("#register-link");
    await expect(registerLink).toHaveAttribute("data-visible", "", {
      timeout: 5000,
    });
  });
});
