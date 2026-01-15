import { test, expect, APP_PATH, addVirtualAuthenticator } from "./fixtures.ts";

test.describe("Login page", () => {
  test("page loads with correct title", async ({ page, baseUrl }) => {
    await page.goto(`${baseUrl}/login/index.html`);

    await expect(page).toHaveTitle("Crowchiper");
  });

  test("button enabled after JS loads", async ({ page, baseUrl }) => {
    await page.goto(`${baseUrl}/login/index.html`);

    const loginButton = page.locator("#login-button");
    await expect(loginButton).toBeEnabled({ timeout: 5000 });
  });

  test("username input visible and functional", async ({ page, baseUrl }) => {
    await page.goto(`${baseUrl}/login/index.html`);

    // Wait for JS to load
    await expect(page.locator("#login-button")).toBeEnabled({ timeout: 5000 });

    // Verify username input is visible
    const usernameInput = page.locator("#username");
    await expect(usernameInput).toBeVisible();

    // Type in the input and verify value
    await usernameInput.fill("testuser");
    await expect(usernameInput).toHaveValue("testuser");
  });

  test("register link navigates to register page", async ({
    page,
    baseUrl,
  }) => {
    await page.goto(`${baseUrl}/login/index.html`);

    // Wait for register link to become visible (after config fetch)
    const registerLink = page.locator("#register-link");
    await expect(registerLink).toHaveAttribute("data-visible", "", {
      timeout: 5000,
    });

    // Click register link
    await registerLink.click();

    // Verify navigation to register page
    await expect(page).toHaveTitle("Register - Crowchiper", { timeout: 5000 });
  });

  test("login success redirects to encryption setup", async ({
    context,
    baseUrl,
  }) => {
    // Create a fresh page for registration
    const page = await context.newPage();
    const client = await page.context().newCDPSession(page);
    await addVirtualAuthenticator(client);

    // First, register a user via the register page
    await page.goto(`${baseUrl}/login/register.html`);
    await expect(page.locator("#register-button")).toBeEnabled({
      timeout: 5000,
    });

    await page.fill("#username", "loginuser");
    await page.click("#register-button");

    // Wait for redirect to encryption setup after registration
    await expect(page).toHaveURL(
      new RegExp(`${APP_PATH}/setup-encryption.html`),
      {
        timeout: 10000,
      },
    );

    // Clear cookies via CDP (works for HttpOnly cookies too)
    await client.send("Network.clearBrowserCookies");

    // Navigate to login page
    await page.goto(`${baseUrl}/login/index.html`);

    // Wait for login button to be enabled
    await expect(page.locator("#login-button")).toBeEnabled({ timeout: 5000 });

    // Fill username and click login
    await page.fill("#username", "loginuser");
    await page.click("#login-button");

    // Wait for redirect to encryption setup (user hasn't set up encryption yet)
    await expect(page).toHaveURL(
      new RegExp(`${APP_PATH}/setup-encryption.html`),
      {
        timeout: 10000,
      },
    );

    await page.close();
  });
});

test.describe("Login with base path", () => {
  test("login redirects to app with base path", async ({
    context,
    serverWithOptions,
  }) => {
    const { baseUrl } = await serverWithOptions({ base: "/myapp" });

    // Create a fresh page
    const page = await context.newPage();
    const client = await page.context().newCDPSession(page);
    await addVirtualAuthenticator(client);

    // First, register a user
    await page.goto(`${baseUrl}/login/register.html`);
    await expect(page.locator("#register-button")).toBeEnabled({
      timeout: 5000,
    });

    await page.fill("#username", "baseuser");
    await page.click("#register-button");

    // Registration redirects directly to encryption setup with base path
    await expect(page).toHaveURL(
      new RegExp(`/myapp${APP_PATH}/setup-encryption.html`),
      {
        timeout: 10000,
      },
    );

    // Verify URL contains both base path and app path
    const url = page.url();
    expect(url).toContain("/myapp");
    expect(url).toContain(APP_PATH);
    expect(url).toContain("setup-encryption.html");

    // Clear cookies via CDP (works for HttpOnly cookies too)
    await client.send("Network.clearBrowserCookies");

    // Navigate to login page
    await page.goto(`${baseUrl}/login/index.html`);

    // Wait for login button to be enabled
    await expect(page.locator("#login-button")).toBeEnabled({ timeout: 5000 });

    // Fill username and click login
    await page.fill("#username", "baseuser");
    await page.click("#login-button");

    // Wait for redirect to encryption setup with base path
    await expect(page).toHaveURL(
      new RegExp(`/myapp${APP_PATH}/setup-encryption.html`),
      {
        timeout: 10000,
      },
    );

    // Verify URL again after login
    const loginUrl = page.url();
    expect(loginUrl).toContain("/myapp");
    expect(loginUrl).toContain(APP_PATH);
    expect(loginUrl).toContain("setup-encryption.html");

    await page.close();
  });
});

test.describe("Login with signup disabled", () => {
  test("register link hidden when signup disabled", async ({
    page,
    serverWithOptions,
  }) => {
    const { baseUrl } = await serverWithOptions({ noSignup: true });

    await page.goto(`${baseUrl}/login/index.html`);

    // Wait for JS to load
    await expect(page.locator("#login-button")).toBeEnabled({ timeout: 5000 });

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
    await expect(page.locator("#login-button")).toBeEnabled({ timeout: 5000 });

    // Wait for register link to become visible (after config fetch)
    const registerLink = page.locator("#register-link");
    await expect(registerLink).toHaveAttribute("data-visible", "", {
      timeout: 5000,
    });

    // Verify it's visible via CSS
    await expect(registerLink).toBeVisible();
  });
});
