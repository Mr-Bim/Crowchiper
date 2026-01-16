import { test, expect, APP_PATH, Server } from "../utils/fixtures.ts";

test.describe("Base path tests", () => {
  test("login page loads with base path", async ({ page, getServerUrl }) => {
    const baseUrl = await getServerUrl(Server.BasePath);
    await page.goto(`${baseUrl}/login/index.html`);

    await expect(page).toHaveTitle("Crowchiper");
  });

  test("register page loads with base path", async ({ page, getServerUrl }) => {
    const baseUrl = await getServerUrl(Server.BasePath);
    await page.goto(`${baseUrl}/login/register.html`);

    await expect(page).toHaveTitle("Register - Crowchiper");
  });

  test("assets load with base path", async ({ page, getServerUrl }) => {
    const baseUrl = await getServerUrl(Server.BasePath);
    await page.goto(`${baseUrl}/login/index.html`);

    // Wait for JS to load - login button should be enabled
    const loginButton = page.locator("#login-button");
    await expect(loginButton).toBeEnabled({ timeout: 5000 });
  });

  test("navigation works with base path", async ({ page, getServerUrl }) => {
    const baseUrl = await getServerUrl(Server.BasePath);
    await page.goto(`${baseUrl}/login/index.html`);

    // Wait for JS to load and register link to become visible
    const registerLink = page.locator("#register-link");
    await expect(registerLink).toHaveAttribute("data-visible", "", {
      timeout: 5000,
    });

    // Click the register link
    await registerLink.click();

    // Verify navigation to register page
    await expect(page).toHaveTitle("Register - Crowchiper", { timeout: 5000 });

    // Verify URL contains the base path (/crow-chipher)
    const url = page.url();
    expect(url).toContain("/crow-chipher");
    expect(url).toContain("/login/register.html");
  });

  test("theme works with base path", async ({ page, getServerUrl }) => {
    const baseUrl = await getServerUrl(Server.BasePath);
    await page.goto(`${baseUrl}/login/index.html`);

    // Wait for theme select to be created
    const themeSelect = page.locator("#theme-select");
    await expect(themeSelect).toBeVisible({ timeout: 5000 });

    // Get initial theme
    const initialTheme = await page.evaluate(
      () => document.documentElement.getAttribute("data-theme") || "",
    );

    // Select a different theme
    const newTheme =
      initialTheme === "warm-light" ? "scandi-dark" : "warm-light";
    await themeSelect.selectOption(newTheme);

    // Verify theme changed
    const currentTheme = await page.evaluate(
      () => document.documentElement.getAttribute("data-theme") || "",
    );
    expect(currentTheme).toBe(newTheme);
  });

  test("app redirect works with base path", async ({
    context,
    getServerUrl,
    testId,
  }) => {
    const baseUrl = await getServerUrl(Server.BasePath);

    // Create a fresh page with WebAuthn
    const page = await context.newPage();
    const client = await page.context().newCDPSession(page);
    await client.send("WebAuthn.enable");
    await client.send("WebAuthn.addVirtualAuthenticator", {
      options: {
        protocol: "ctap2",
        transport: "internal",
        hasResidentKey: true,
        hasUserVerification: true,
        isUserVerified: true,
        hasPrf: true,
        automaticPresenceSimulation: true,
      },
    });

    const username = `basepath_${testId}`;

    // Register a user
    await page.goto(`${baseUrl}/login/register.html`);
    await expect(page.locator("#register-button")).toBeEnabled({
      timeout: 5000,
    });

    await page.fill("#username", username);
    await page.click("#register-button");

    // Should redirect to encryption setup with base path
    await expect(page).toHaveURL(
      new RegExp(`/crow-chipher${APP_PATH}/setup-encryption.html`),
      { timeout: 10000 },
    );

    // Verify both base path and app path are in URL
    const url = page.url();
    expect(url).toContain("/crow-chipher");
    expect(url).toContain(APP_PATH);

    await page.close();
  });
});
