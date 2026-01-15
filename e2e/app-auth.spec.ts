import * as jose from "jose";
import { test, expect, appUrl, APP_PATH } from "./fixtures.ts";

// JWT helper utilities
const JWT_SECRET = new TextEncoder().encode(
  "test-jwt-secret-for-playwright-testing-minimum-32-chars",
);
const TOKEN_DURATION_SECS = 3600;

enum UserRole {
  User = "user",
  Admin = "admin",
}

interface Claims {
  sub: string;
  username: string;
  role: UserRole;
  iat: number;
  exp: number;
}

async function generateToken(
  userUuid: string,
  username: string,
  role: UserRole,
): Promise<string> {
  const now = Math.floor(Date.now() / 1000);

  const claims: Claims = {
    sub: userUuid,
    username: username,
    role: role,
    iat: now,
    exp: now + TOKEN_DURATION_SECS,
  };

  return await new jose.SignJWT(claims as unknown as jose.JWTPayload)
    .setProtectedHeader({ alg: "HS256" })
    .sign(JWT_SECRET);
}

async function generateExpiredToken(
  userUuid: string,
  username: string,
  role: UserRole,
): Promise<string> {
  const now = Math.floor(Date.now() / 1000);

  const claims: Claims = {
    sub: userUuid,
    username: username,
    role: role,
    iat: now - 100,
    exp: now - 50, // Expired 50 seconds ago
  };

  return await new jose.SignJWT(claims as unknown as jose.JWTPayload)
    .setProtectedHeader({ alg: "HS256" })
    .sign(JWT_SECRET);
}

test.describe("App authentication", () => {
  test("app redirects to login without token", async ({ page, baseUrl }) => {
    // Navigate to login first to clear any cookies
    await page.goto(`${baseUrl}/login/index.html`);
    await page.evaluate(
      "document.cookie = 'auth_token=; path=/; expires=Thu, 01 Jan 1970 00:00:00 GMT'",
    );

    // Try to access app without token - should redirect to login
    await page.goto(`${appUrl(baseUrl)}/index.html`);

    // Wait for redirect to complete
    await expect(page).toHaveURL(/\/login/, { timeout: 5000 });
  });

  test("app accessible with valid token", async ({ page, baseUrl }) => {
    // Generate a valid token
    const token = await generateToken("test-uuid", "testuser", UserRole.User);

    // First navigate to login page so we can set cookies on the domain
    await page.goto(`${baseUrl}/login/index.html`);

    // Set the cookie
    await page.evaluate((t) => {
      document.cookie = `auth_token=${t}; path=/`;
    }, token);

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

    // Set an invalid token
    await page.evaluate("document.cookie = 'auth_token=invalid-token; path=/'");

    // Navigate to app
    await page.goto(`${appUrl(baseUrl)}/index.html`);

    // Should redirect to login
    await expect(page).toHaveURL(/\/login/, { timeout: 5000 });
  });

  test("app redirects with expired token", async ({ context, baseUrl }) => {
    // Use a fresh page to avoid cookie contamination
    const page = await context.newPage();

    // Create an expired token
    const token = await generateExpiredToken(
      "test-uuid",
      "testuser",
      UserRole.User,
    );

    // First navigate to login page so we can set cookies on the domain
    await page.goto(`${baseUrl}/login/index.html`);

    // Set the expired token
    await page.evaluate((t) => {
      document.cookie = `auth_token=${t}; path=/`;
    }, token);

    // Navigate to app
    await page.goto(`${appUrl(baseUrl)}/index.html`);

    // Should redirect to login
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
    // Generate an admin token
    const token = await generateToken("admin-uuid", "admin", UserRole.Admin);

    // First navigate to login page so we can set cookies
    await page.goto(`${baseUrl}/login/index.html`);

    // Set the cookie
    await page.evaluate((t) => {
      document.cookie = `auth_token=${t}; path=/`;
    }, token);

    // Navigate to app
    await page.goto(`${appUrl(baseUrl)}/index.html`);

    // Should stay on app page
    await expect(page).toHaveURL(new RegExp(APP_PATH), { timeout: 5000 });
  });
});

test.describe("App authentication with base path", () => {
  test("app with base path redirects to login", async ({
    page,
    serverWithOptions,
  }) => {
    const { baseUrl } = await serverWithOptions({ base: "/myapp" });

    // Navigate to login first to clear any cookies
    await page.goto(`${baseUrl}/login/index.html`);
    await page.evaluate(
      "document.cookie = 'auth_token=; path=/; expires=Thu, 01 Jan 1970 00:00:00 GMT'",
    );

    // Try to access app without token
    await page.goto(`${appUrl(baseUrl)}/index.html`);

    // Should redirect to login
    await expect(page).toHaveURL(/\/login/, { timeout: 5000 });
  });

  test("app with base path accessible with token", async ({
    page,
    serverWithOptions,
  }) => {
    const { baseUrl } = await serverWithOptions({ base: "/crowchiper" });

    // Generate a valid token
    const token = await generateToken("test-uuid", "testuser", UserRole.User);

    // Navigate to login first to set cookie
    await page.goto(`${baseUrl}/login/index.html`);

    // Set the cookie
    await page.evaluate((t) => {
      document.cookie = `auth_token=${t}; path=/`;
    }, token);

    // Navigate to app
    await page.goto(`${appUrl(baseUrl)}/index.html`);

    // Should stay on app page
    await expect(page).toHaveURL(new RegExp(APP_PATH), { timeout: 5000 });
  });
});
