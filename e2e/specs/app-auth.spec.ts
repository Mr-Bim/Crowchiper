import { test, expect, appUrl, APP_PATH, Server } from "../utils/fixtures.ts";

interface GenerateTokensResponse {
  access_token: string;
  refresh_token: string;
  refresh_jti: string;
  issued_at: number;
  expires_at: number;
}

/** Generate tokens via the test API endpoint */
async function generateTokens(
  baseUrl: string,
  userUuid: string,
  username: string,
  options: {
    role?: "user" | "admin";
    ip_addr?: string;
    expired_access?: boolean;
    store_refresh?: boolean;
  } = {},
): Promise<GenerateTokensResponse> {
  const response = await fetch(`${baseUrl}/api/test/generate-tokens`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({
      user_uuid: userUuid,
      username,
      role: options.role,
      ip_addr: options.ip_addr,
      expired_access: options.expired_access ?? false,
      store_refresh: options.store_refresh ?? false,
    }),
  });
  if (!response.ok) {
    throw new Error(`Failed to generate tokens: ${response.status}`);
  }
  return response.json();
}

test.describe("App authentication", () => {
  test("app redirects to login without token", async ({ page, baseUrl }) => {
    // Navigate to login first to clear any cookies
    await page.goto(`${baseUrl}/login/index.html`);
    await page.evaluate(
      "document.cookie = 'access_token=; path=/; expires=Thu, 01 Jan 1970 00:00:00 GMT'",
    );
    await page.evaluate(
      "document.cookie = 'refresh_token=; path=/; expires=Thu, 01 Jan 1970 00:00:00 GMT'",
    );

    // Try to access app without token - should redirect to login
    await page.goto(`${appUrl(baseUrl)}/index.html`);

    // Wait for redirect to complete
    await expect(page).toHaveURL(/\/login/, { timeout: 5000 });
  });

  test("app accessible with valid token", async ({ page, baseUrl }) => {
    const userUuid = crypto.randomUUID();
    const username = "testuser";

    // Generate both access and refresh tokens via test API
    const tokens = await generateTokens(baseUrl, userUuid, username, {
      store_refresh: true,
    });

    // First navigate to login page so we can set cookies on the domain
    await page.goto(`${baseUrl}/login/index.html`);

    // Set both cookies
    await page.evaluate(
      ({ access, refresh }) => {
        document.cookie = `access_token=${access}; path=/`;
        document.cookie = `refresh_token=${refresh}; path=/`;
      },
      { access: tokens.access_token, refresh: tokens.refresh_token },
    );

    // Now navigate to app
    await page.goto(`${appUrl(baseUrl)}/index.html`);

    // Should stay on app page (not redirected)
    await expect(page).toHaveURL(new RegExp(APP_PATH), { timeout: 5000 });

    // Verify the page title
    await expect(page).toHaveTitle("Crowchiper");
  });

  test("app redirects with invalid token", async ({ page, baseUrl }) => {
    // First navigate to login page so we can set cookies
    await page.goto(`${baseUrl}/login/index.html`);

    // Set invalid tokens
    await page.evaluate(
      "document.cookie = 'access_token=invalid-token; path=/'",
    );
    await page.evaluate(
      "document.cookie = 'refresh_token=invalid-token; path=/'",
    );

    // Navigate to app
    await page.goto(`${appUrl(baseUrl)}/index.html`);

    // Should redirect to login
    await expect(page).toHaveURL(/\/login/, { timeout: 5000 });
  });

  test("app redirects with expired token and no refresh token", async ({
    context,
    baseUrl,
  }) => {
    // Use a fresh page to avoid cookie contamination
    const page = await context.newPage();

    // Create an expired access token via test API
    const tokens = await generateTokens(baseUrl, "test-uuid", "testuser", {
      expired_access: true,
      store_refresh: false,
    });

    // First navigate to login page so we can set cookies on the domain
    await page.goto(`${baseUrl}/login/index.html`);

    // Set only the expired access token (no refresh token)
    await page.evaluate((t) => {
      document.cookie = `access_token=${t}; path=/`;
      // Clear any refresh token
      document.cookie =
        "refresh_token=; path=/; expires=Thu, 01 Jan 1970 00:00:00 GMT";
    }, tokens.access_token);

    // Navigate to app
    await page.goto(`${appUrl(baseUrl)}/index.html`);

    // Should redirect to login (no valid refresh token to auto-refresh)
    await expect(page).toHaveURL(/\/login/, { timeout: 5000 });

    await page.close();
  });

  test("app root protected", async ({ page, baseUrl }) => {
    // Try to access app root without token
    await page.goto(`${appUrl(baseUrl)}/`);

    // Should redirect to login
    await expect(page).toHaveURL(/\/login/, { timeout: 5000 });
  });

  test("login assets accessible without token", async ({ page, baseUrl }) => {
    // Login page should be accessible without token
    await page.goto(`${baseUrl}/login/index.html`);

    await expect(page).toHaveTitle("Crowchiper");

    // Register page should also be accessible
    await page.goto(`${baseUrl}/login/register.html`);

    await expect(page).toHaveTitle("Register - Crowchiper");
  });

  test("root redirects to login", async ({ page, baseUrl }) => {
    await page.goto(baseUrl);

    // Should redirect to login
    await expect(page).toHaveURL(/\/login/, { timeout: 5000 });
  });

  test("admin token can access app", async ({ page, baseUrl }) => {
    const userUuid = crypto.randomUUID();
    const username = "admin";

    // Generate both access and refresh tokens for admin via test API
    const tokens = await generateTokens(baseUrl, userUuid, username, {
      role: "admin",
      store_refresh: true,
    });

    // First navigate to login page so we can set cookies
    await page.goto(`${baseUrl}/login/index.html`);

    // Set both cookies
    await page.evaluate(
      ({ access, refresh }) => {
        document.cookie = `access_token=${access}; path=/`;
        document.cookie = `refresh_token=${refresh}; path=/`;
      },
      { access: tokens.access_token, refresh: tokens.refresh_token },
    );

    // Navigate to app
    await page.goto(`${appUrl(baseUrl)}/index.html`);

    // Should stay on app page
    await expect(page).toHaveURL(new RegExp(APP_PATH), { timeout: 5000 });
  });
});

test.describe("App authentication with base path", () => {
  test("app with base path redirects to login", async ({
    page,
    getServerUrl,
  }) => {
    const baseUrl = await getServerUrl(Server.BasePath);

    // Navigate to login first to clear any cookies
    await page.goto(`${baseUrl}/login/index.html`);
    await page.evaluate(
      "document.cookie = 'access_token=; path=/; expires=Thu, 01 Jan 1970 00:00:00 GMT'",
    );
    await page.evaluate(
      "document.cookie = 'refresh_token=; path=/; expires=Thu, 01 Jan 1970 00:00:00 GMT'",
    );

    // Try to access app without token
    await page.goto(`${appUrl(baseUrl)}/index.html`);

    // Should redirect to login
    await expect(page).toHaveURL(/\/login/, { timeout: 5000 });
  });

  test("app with base path accessible with token", async ({
    page,
    getServerUrl,
  }) => {
    const baseUrl = await getServerUrl(Server.BasePath);
    const userUuid = crypto.randomUUID();
    const username = "testuser";

    // Generate both access and refresh tokens via test API
    const tokens = await generateTokens(baseUrl, userUuid, username, {
      store_refresh: true,
    });

    // Navigate to login first to set cookie
    await page.goto(`${baseUrl}/login/index.html`);

    // Set both cookies
    await page.evaluate(
      ({ access, refresh }) => {
        document.cookie = `access_token=${access}; path=/`;
        document.cookie = `refresh_token=${refresh}; path=/`;
      },
      { access: tokens.access_token, refresh: tokens.refresh_token },
    );

    // Navigate to app
    await page.goto(`${appUrl(baseUrl)}/index.html`);

    // Should stay on app page
    await expect(page).toHaveURL(new RegExp(APP_PATH), { timeout: 5000 });
  });
});
