import * as jose from "jose";
import { test, expect, appUrl, APP_PATH, Server } from "../utils/fixtures.ts";

// JWT helper utilities
const JWT_SECRET = new TextEncoder().encode(
  "test-jwt-secret-for-playwright-testing-minimum-32-chars",
);
const ACCESS_TOKEN_DURATION_SECS = 5 * 60; // 5 minutes
const REFRESH_TOKEN_DURATION_SECS = 14 * 24 * 60 * 60; // 2 weeks

enum UserRole {
  User = "user",
  Admin = "admin",
}

enum TokenType {
  Access = "access",
  Refresh = "refresh",
}

interface AccessTokenResult {
  token: string;
  iat: number;
  exp: number;
}

interface RefreshTokenResult {
  token: string;
  jti: string;
  iat: number;
  exp: number;
}

async function generateAccessToken(
  userUuid: string,
  username: string,
  role: UserRole,
): Promise<AccessTokenResult> {
  const now = Math.floor(Date.now() / 1000);
  const exp = now + ACCESS_TOKEN_DURATION_SECS;

  const claims = {
    sub: userUuid,
    username: username,
    role: role,
    typ: TokenType.Access,
    iat: now,
    exp: exp,
  };

  const token = await new jose.SignJWT(claims as unknown as jose.JWTPayload)
    .setProtectedHeader({ alg: "HS256" })
    .sign(JWT_SECRET);

  return { token, iat: now, exp };
}

async function generateRefreshToken(
  userUuid: string,
  username: string,
  role: UserRole,
): Promise<RefreshTokenResult> {
  const now = Math.floor(Date.now() / 1000);
  const jti = crypto.randomUUID();
  const exp = now + REFRESH_TOKEN_DURATION_SECS;

  const claims = {
    jti: jti,
    sub: userUuid,
    username: username,
    role: role,
    typ: TokenType.Refresh,
    iat: now,
    exp: exp,
  };

  const token = await new jose.SignJWT(claims as unknown as jose.JWTPayload)
    .setProtectedHeader({ alg: "HS256" })
    .sign(JWT_SECRET);

  return { token, jti, iat: now, exp };
}

async function generateExpiredAccessToken(
  userUuid: string,
  username: string,
  role: UserRole,
): Promise<string> {
  const now = Math.floor(Date.now() / 1000);

  const claims = {
    sub: userUuid,
    username: username,
    role: role,
    typ: TokenType.Access,
    iat: now - 100,
    exp: now - 50, // Expired 50 seconds ago
  };

  return await new jose.SignJWT(claims as unknown as jose.JWTPayload)
    .setProtectedHeader({ alg: "HS256" })
    .sign(JWT_SECRET);
}

/** Store refresh token in backend via test API */
async function storeRefreshToken(
  baseUrl: string,
  jti: string,
  userUuid: string,
  username: string,
  iat: number,
  exp: number,
): Promise<void> {
  const response = await fetch(`${baseUrl}/api/test/token`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({
      jti,
      user_uuid: userUuid,
      username,
      issued_at: iat,
      expires_at: exp,
    }),
  });
  if (!response.ok) {
    throw new Error(`Failed to store token: ${response.status}`);
  }
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

    // Generate both access and refresh tokens
    const accessResult = await generateAccessToken(
      userUuid,
      username,
      UserRole.User,
    );
    const refreshResult = await generateRefreshToken(
      userUuid,
      username,
      UserRole.User,
    );

    // Store the refresh token in the backend (access tokens are stateless)
    await storeRefreshToken(
      baseUrl,
      refreshResult.jti,
      userUuid,
      username,
      refreshResult.iat,
      refreshResult.exp,
    );

    // First navigate to login page so we can set cookies on the domain
    await page.goto(`${baseUrl}/login/index.html`);

    // Set both cookies
    await page.evaluate(
      ({ access, refresh }) => {
        document.cookie = `access_token=${access}; path=/`;
        document.cookie = `refresh_token=${refresh}; path=/`;
      },
      { access: accessResult.token, refresh: refreshResult.token },
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

    // Create an expired access token
    const token = await generateExpiredAccessToken(
      "test-uuid",
      "testuser",
      UserRole.User,
    );

    // First navigate to login page so we can set cookies on the domain
    await page.goto(`${baseUrl}/login/index.html`);

    // Set only the expired access token (no refresh token)
    await page.evaluate((t) => {
      document.cookie = `access_token=${t}; path=/`;
      // Clear any refresh token
      document.cookie =
        "refresh_token=; path=/; expires=Thu, 01 Jan 1970 00:00:00 GMT";
    }, token);

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

    // Generate both access and refresh tokens for admin
    const accessResult = await generateAccessToken(
      userUuid,
      username,
      UserRole.Admin,
    );
    const refreshResult = await generateRefreshToken(
      userUuid,
      username,
      UserRole.Admin,
    );

    // Store the refresh token in the backend
    await storeRefreshToken(
      baseUrl,
      refreshResult.jti,
      userUuid,
      username,
      refreshResult.iat,
      refreshResult.exp,
    );

    // First navigate to login page so we can set cookies
    await page.goto(`${baseUrl}/login/index.html`);

    // Set both cookies
    await page.evaluate(
      ({ access, refresh }) => {
        document.cookie = `access_token=${access}; path=/`;
        document.cookie = `refresh_token=${refresh}; path=/`;
      },
      { access: accessResult.token, refresh: refreshResult.token },
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

    // Generate both access and refresh tokens
    const accessResult = await generateAccessToken(
      userUuid,
      username,
      UserRole.User,
    );
    const refreshResult = await generateRefreshToken(
      userUuid,
      username,
      UserRole.User,
    );

    // Store the refresh token in the backend
    await storeRefreshToken(
      baseUrl,
      refreshResult.jti,
      userUuid,
      username,
      refreshResult.iat,
      refreshResult.exp,
    );

    // Navigate to login first to set cookie
    await page.goto(`${baseUrl}/login/index.html`);

    // Set both cookies
    await page.evaluate(
      ({ access, refresh }) => {
        document.cookie = `access_token=${access}; path=/`;
        document.cookie = `refresh_token=${refresh}; path=/`;
      },
      { access: accessResult.token, refresh: refreshResult.token },
    );

    // Navigate to app
    await page.goto(`${appUrl(baseUrl)}/index.html`);

    // Should stay on app page
    await expect(page).toHaveURL(new RegExp(APP_PATH), { timeout: 5000 });
  });
});
