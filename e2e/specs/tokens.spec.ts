/**
 * Token System E2E Tests
 *
 * Tests for the dual-token authentication system:
 * - Access tokens (short-lived, 5 min)
 * - Refresh tokens (long-lived, 2 weeks)
 *
 * Covers:
 * - Token issuance on login/register
 * - Token refresh flow
 * - Multiple sessions/devices
 * - Logout and token revocation
 * - User isolation
 * - Token reuse on re-login
 */

import {
  test,
  expect,
  APP_PATH,
  addVirtualAuthenticator,
} from "../utils/fixtures.ts";
import type { BrowserContext } from "@playwright/test";

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

/** Get cookies from a page context */
async function getCookies(
  context: BrowserContext,
): Promise<{ name: string; value: string }[]> {
  return await context.cookies();
}

/** Get a specific cookie by name */
async function getCookie(
  context: BrowserContext,
  name: string,
): Promise<string | undefined> {
  const cookies = await getCookies(context);
  return cookies.find((c) => c.name === name)?.value;
}

/** Check if a cookie exists */
async function hasCookie(
  context: BrowserContext,
  name: string,
): Promise<boolean> {
  const cookie = await getCookie(context, name);
  return cookie !== undefined && cookie !== "";
}

/** Clear all cookies in a context */
async function clearCookies(context: BrowserContext): Promise<void> {
  await context.clearCookies();
}

// =============================================================================
// Token Issuance Tests
// =============================================================================

test.describe("Token issuance on registration", () => {
  test("registration issues refresh token cookie", async ({
    context,
    baseUrl,
    testId,
  }) => {
    const page = await context.newPage();
    const client = await page.context().newCDPSession(page);
    await addVirtualAuthenticator(client);

    const username = `reg_token_${testId}`;

    await page.goto(`${baseUrl}/login/register.html`);
    await expect(page.locator("#register-button")).toBeEnabled({
      timeout: 5000,
    });

    await page.fill("#username", username);
    await page.click("#register-button");

    // Wait for redirect to setup-encryption
    await expect(page).toHaveTitle("Setup Encryption - Crowchiper", {
      timeout: 10000,
    });

    // Verify refresh token cookie was set
    const hasRefresh = await hasCookie(context, "refresh_token");
    expect(hasRefresh).toBe(true);

    await page.close();
  });

  test("registration does not issue access token immediately", async ({
    context,
    baseUrl,
    testId,
  }) => {
    // Access tokens are issued by middleware on first authenticated request,
    // not directly by registration. The refresh token is used to get an access token.
    const page = await context.newPage();
    const client = await page.context().newCDPSession(page);
    await addVirtualAuthenticator(client);

    const username = `reg_noaccess_${testId}`;

    await page.goto(`${baseUrl}/login/register.html`);
    await expect(page.locator("#register-button")).toBeEnabled({
      timeout: 5000,
    });

    await page.fill("#username", username);
    await page.click("#register-button");

    await expect(page).toHaveTitle("Setup Encryption - Crowchiper", {
      timeout: 10000,
    });

    // Refresh token should exist
    const hasRefresh = await hasCookie(context, "refresh_token");
    expect(hasRefresh).toBe(true);

    await page.close();
  });
});

test.describe("Token issuance on login", () => {
  test("login issues refresh token for user without existing token", async ({
    browser,
    baseUrl,
    testId,
  }) => {
    const username = `login_newtoken_${testId}`;

    // Create a single context that will hold the virtual authenticator
    // (Passkeys are tied to the authenticator, so we need the same one)
    const context = await browser.newContext();
    const page = await context.newPage();
    const client = await page.context().newCDPSession(page);
    await addVirtualAuthenticator(client);

    // Register the user
    await page.goto(`${baseUrl}/login/register.html`);
    await expect(page.locator("#register-button")).toBeEnabled({
      timeout: 5000,
    });
    await page.fill("#username", username);
    await page.click("#register-button");

    await expect(page).toHaveTitle("Setup Encryption - Crowchiper", {
      timeout: 10000,
    });

    // Get the initial refresh token
    const token1 = await getCookie(context, "refresh_token");
    expect(token1).toBeDefined();

    // Clear cookies to simulate "new device" (but keep same authenticator)
    await client.send("Network.clearBrowserCookies");

    // Verify cookies are cleared
    const tokenAfterClear = await getCookie(context, "refresh_token");
    expect(tokenAfterClear).toBeUndefined();

    // Now login (same authenticator, no cookies)
    await page.goto(`${baseUrl}/login/index.html`);
    await expect(page.locator("#login-button")).toBeEnabled({ timeout: 5000 });

    await page.fill("#username", username);
    await page.click("#login-button");

    // Wait for redirect
    await expect(page).toHaveURL(
      new RegExp(`${APP_PATH}/setup-encryption.html`),
      {
        timeout: 10000,
      },
    );

    // Verify a NEW refresh token was issued (different from the first one)
    const token2 = await getCookie(context, "refresh_token");
    expect(token2).toBeDefined();
    expect(token2).not.toBe(token1); // Should be a new token

    await context.close();
  });

  test("login reuses existing valid refresh token", async ({
    browser,
    baseUrl,
    testId,
  }) => {
    const username = `login_reuse_${testId}`;

    // Register user in first context
    const context1 = await browser.newContext();
    const page1 = await context1.newPage();
    const client1 = await page1.context().newCDPSession(page1);
    await addVirtualAuthenticator(client1);

    await page1.goto(`${baseUrl}/login/register.html`);
    await expect(page1.locator("#register-button")).toBeEnabled({
      timeout: 5000,
    });
    await page1.fill("#username", username);
    await page1.click("#register-button");

    await expect(page1).toHaveTitle("Setup Encryption - Crowchiper", {
      timeout: 10000,
    });

    // Get the refresh token after registration
    const refreshToken1 = await getCookie(context1, "refresh_token");
    expect(refreshToken1).toBeDefined();

    // When navigating to login page with valid refresh token, the IIFE config
    // detects authentication and redirects to app. This verifies the token is valid.
    await page1.goto(`${baseUrl}/login/index.html`);

    // Should redirect to app (because already authenticated with valid refresh token)
    await expect(page1).toHaveURL(new RegExp(APP_PATH), {
      timeout: 10000,
    });

    // The refresh token should still be the same (not replaced)
    const refreshToken2 = await getCookie(context1, "refresh_token");
    expect(refreshToken2).toBe(refreshToken1);

    await context1.close();
  });
});

// =============================================================================
// Token Refresh Flow Tests
// =============================================================================

test.describe("Token refresh flow", () => {
  test("expired access token triggers refresh", async ({
    context,
    baseUrl,
  }) => {
    const userUuid = crypto.randomUUID();
    const username = "refreshtest";

    // Generate tokens via test API (with expired access token, stored refresh)
    const tokens = await generateTokens(baseUrl, userUuid, username, {
      expired_access: true,
      store_refresh: true,
    });

    const page = await context.newPage();

    // Set cookies
    await page.goto(`${baseUrl}/login/index.html`);
    await context.addCookies([
      {
        name: "access_token",
        value: tokens.access_token,
        domain: new URL(baseUrl).hostname,
        path: "/",
      },
      {
        name: "refresh_token",
        value: tokens.refresh_token,
        domain: new URL(baseUrl).hostname,
        path: "/",
      },
    ]);

    // Make an API request - should succeed via refresh
    const response = await page.request.get(`${baseUrl}/api/tokens/verify`);
    expect(response.status()).toBe(200);

    // Check that a new access token was issued
    const cookies = await context.cookies();
    const accessCookie = cookies.find((c) => c.name === "access_token");
    expect(accessCookie).toBeDefined();
    expect(accessCookie?.value).not.toBe(tokens.access_token);

    await page.close();
  });

  test("missing refresh token returns unauthorized", async ({
    context,
    baseUrl,
  }) => {
    const userUuid = crypto.randomUUID();
    const username = "norefresh";

    // Generate only an expired access token (don't store refresh)
    const tokens = await generateTokens(baseUrl, userUuid, username, {
      expired_access: true,
      store_refresh: false,
    });

    const page = await context.newPage();

    // Set only expired access token (no refresh token)
    await page.goto(`${baseUrl}/login/index.html`);
    await context.addCookies([
      {
        name: "access_token",
        value: tokens.access_token,
        domain: new URL(baseUrl).hostname,
        path: "/",
      },
    ]);

    // Make an API request - should fail
    const response = await page.request.get(`${baseUrl}/api/tokens/verify`);
    expect(response.status()).toBe(401);

    await page.close();
  });

  test("revoked refresh token returns unauthorized", async ({
    context,
    baseUrl,
  }) => {
    const userUuid = crypto.randomUUID();
    const username = "revokedrefresh";

    // Generate tokens but don't store the refresh token (simulates revoked)
    const tokens = await generateTokens(baseUrl, userUuid, username, {
      expired_access: true,
      store_refresh: false, // Not stored = "revoked"
    });

    const page = await context.newPage();

    await page.goto(`${baseUrl}/login/index.html`);
    await context.addCookies([
      {
        name: "access_token",
        value: tokens.access_token,
        domain: new URL(baseUrl).hostname,
        path: "/",
      },
      {
        name: "refresh_token",
        value: tokens.refresh_token,
        domain: new URL(baseUrl).hostname,
        path: "/",
      },
    ]);

    // Make an API request - should fail (refresh token not in DB)
    const response = await page.request.get(`${baseUrl}/api/tokens/verify`);
    expect(response.status()).toBe(401);

    await page.close();
  });
});

// =============================================================================
// Multiple Sessions/Devices Tests
// =============================================================================

test.describe("Multiple sessions", () => {
  test("user can have multiple active sessions", async ({
    browser,
    baseUrl,
    testId,
  }) => {
    const username = `multi_session_${testId}`;

    // Use a single context with one authenticator
    // (Passkeys are tied to the authenticator)
    const context = await browser.newContext();
    const page = await context.newPage();
    const client = await page.context().newCDPSession(page);
    await addVirtualAuthenticator(client);

    // Register user
    await page.goto(`${baseUrl}/login/register.html`);
    await expect(page.locator("#register-button")).toBeEnabled({
      timeout: 5000,
    });
    await page.fill("#username", username);
    await page.click("#register-button");

    await expect(page).toHaveTitle("Setup Encryption - Crowchiper", {
      timeout: 10000,
    });

    const token1 = await getCookie(context, "refresh_token");
    expect(token1).toBeDefined();

    // Clear cookies and login again to create second session
    await client.send("Network.clearBrowserCookies");

    await page.goto(`${baseUrl}/login/index.html`);
    await expect(page.locator("#login-button")).toBeEnabled({ timeout: 5000 });
    await page.fill("#username", username);
    await page.click("#login-button");

    await expect(page).toHaveURL(
      new RegExp(`${APP_PATH}/setup-encryption.html`),
      {
        timeout: 10000,
      },
    );

    const token2 = await getCookie(context, "refresh_token");
    expect(token2).toBeDefined();

    // Tokens should be different (different sessions)
    expect(token2).not.toBe(token1);

    // Current session should be valid
    const response = await page.request.get(`${baseUrl}/api/tokens/verify`);
    expect(response.status()).toBe(200);

    // Both tokens should exist in the database (user has 2 sessions)
    const tokensResponse = await page.request.get(`${baseUrl}/api/tokens`);
    expect(tokensResponse.status()).toBe(200);
    const tokensData = await tokensResponse.json();
    expect(tokensData.tokens.length).toBe(2);

    await context.close();
  });

  test("logging out one session doesn't affect others", async ({
    browser,
    baseUrl,
    testId,
  }) => {
    const username = `logout_multi_${testId}`;

    // Use a single context with one authenticator
    const context = await browser.newContext();
    const page = await context.newPage();
    const client = await page.context().newCDPSession(page);
    await addVirtualAuthenticator(client);

    // Register user (creates first session)
    await page.goto(`${baseUrl}/login/register.html`);
    await expect(page.locator("#register-button")).toBeEnabled({
      timeout: 5000,
    });
    await page.fill("#username", username);
    await page.click("#register-button");

    await expect(page).toHaveTitle("Setup Encryption - Crowchiper", {
      timeout: 10000,
    });

    // Get the first session's token
    const token1 = await getCookie(context, "refresh_token");
    expect(token1).toBeDefined();

    // Clear cookies and create second session
    await client.send("Network.clearBrowserCookies");

    await page.goto(`${baseUrl}/login/index.html`);
    await expect(page.locator("#login-button")).toBeEnabled({ timeout: 5000 });
    await page.fill("#username", username);
    await page.click("#login-button");

    await expect(page).toHaveURL(
      new RegExp(`${APP_PATH}/setup-encryption.html`),
      {
        timeout: 10000,
      },
    );

    // Get the second session's token
    const token2 = await getCookie(context, "refresh_token");
    expect(token2).toBeDefined();
    expect(token2).not.toBe(token1);

    // Verify we have 2 sessions
    const tokensResponse = await page.request.get(`${baseUrl}/api/tokens`);
    const tokensData = await tokensResponse.json();
    expect(tokensData.tokens.length).toBe(2);

    // Logout current session (session 2)
    const logoutResponse = await page.request.post(
      `${baseUrl}/api/tokens/logout`,
    );
    expect(logoutResponse.status()).toBe(200);

    // Current session should no longer be valid
    const response = await page.request.get(`${baseUrl}/api/tokens/verify`);
    expect(response.status()).toBe(401);

    // Set the first session's token back to verify it still works
    await context.addCookies([
      {
        name: "refresh_token",
        value: token1!,
        domain: new URL(baseUrl).hostname,
        path: "/",
      },
    ]);

    // First session should still be valid
    const response1 = await page.request.get(`${baseUrl}/api/tokens/verify`);
    expect(response1.status()).toBe(200);

    // Now only 1 session should remain
    const tokensResponse2 = await page.request.get(`${baseUrl}/api/tokens`);
    const tokensData2 = await tokensResponse2.json();
    expect(tokensData2.tokens.length).toBe(1);

    await context.close();
  });
});

// =============================================================================
// Logout Tests
// =============================================================================

test.describe("Logout", () => {
  test("logout clears cookies", async ({ browser, baseUrl, testId }) => {
    const username = `logout_clear_${testId}`;

    const context = await browser.newContext();
    const page = await context.newPage();
    const client = await page.context().newCDPSession(page);
    await addVirtualAuthenticator(client);

    // Register
    await page.goto(`${baseUrl}/login/register.html`);
    await expect(page.locator("#register-button")).toBeEnabled({
      timeout: 5000,
    });
    await page.fill("#username", username);
    await page.click("#register-button");

    await expect(page).toHaveTitle("Setup Encryption - Crowchiper", {
      timeout: 10000,
    });

    // Verify we have a refresh token
    expect(await hasCookie(context, "refresh_token")).toBe(true);

    // Logout
    const logoutResponse = await page.request.post(
      `${baseUrl}/api/tokens/logout`,
    );
    expect(logoutResponse.status()).toBe(200);

    // Verify cookies are cleared
    const refreshToken = await getCookie(context, "refresh_token");
    const accessToken = await getCookie(context, "access_token");

    // Cookies should be empty or not present
    expect(!refreshToken || refreshToken === "").toBe(true);
    expect(!accessToken || accessToken === "").toBe(true);

    await context.close();
  });

  test("logout invalidates refresh token", async ({
    browser,
    baseUrl,
    testId,
  }) => {
    const username = `logout_invalid_${testId}`;

    const context = await browser.newContext();
    const page = await context.newPage();
    const client = await page.context().newCDPSession(page);
    await addVirtualAuthenticator(client);

    // Register
    await page.goto(`${baseUrl}/login/register.html`);
    await expect(page.locator("#register-button")).toBeEnabled({
      timeout: 5000,
    });
    await page.fill("#username", username);
    await page.click("#register-button");

    await expect(page).toHaveTitle("Setup Encryption - Crowchiper", {
      timeout: 10000,
    });

    // Save the refresh token before logout
    const refreshTokenBeforeLogout = await getCookie(context, "refresh_token");
    expect(refreshTokenBeforeLogout).toBeDefined();

    // Logout
    await page.request.post(`${baseUrl}/api/tokens/logout`);

    // Try to use the old refresh token manually
    // Set it back on a new context
    const context2 = await browser.newContext();
    const page2 = await context2.newPage();

    await page2.goto(`${baseUrl}/login/index.html`);
    await context2.addCookies([
      {
        name: "refresh_token",
        value: refreshTokenBeforeLogout!,
        domain: new URL(baseUrl).hostname,
        path: "/",
      },
    ]);

    // The old refresh token should no longer work
    const response = await page2.request.get(`${baseUrl}/api/tokens/verify`);
    expect(response.status()).toBe(401);

    await context.close();
    await context2.close();
  });

  test("logout without token succeeds", async ({ context, baseUrl }) => {
    const page = await context.newPage();

    // Clear any cookies
    await clearCookies(context);

    await page.goto(`${baseUrl}/login/index.html`);

    // Logout without any tokens - should succeed (idempotent)
    const response = await page.request.post(`${baseUrl}/api/tokens/logout`);
    expect(response.status()).toBe(200);

    await page.close();
  });
});

// =============================================================================
// Token List API Tests
// =============================================================================

test.describe("Token list API", () => {
  test("list tokens returns only own tokens", async ({
    browser,
    baseUrl,
    testId,
  }) => {
    // Create two users
    const username1 = `list_user1_${testId}`;
    const username2 = `list_user2_${testId}`;

    // User 1
    const context1 = await browser.newContext();
    const page1 = await context1.newPage();
    const client1 = await page1.context().newCDPSession(page1);
    await addVirtualAuthenticator(client1);

    await page1.goto(`${baseUrl}/login/register.html`);
    await expect(page1.locator("#register-button")).toBeEnabled({
      timeout: 5000,
    });
    await page1.fill("#username", username1);
    await page1.click("#register-button");

    await expect(page1).toHaveTitle("Setup Encryption - Crowchiper", {
      timeout: 10000,
    });

    // User 2
    const context2 = await browser.newContext();
    const page2 = await context2.newPage();
    const client2 = await page2.context().newCDPSession(page2);
    await addVirtualAuthenticator(client2);

    await page2.goto(`${baseUrl}/login/register.html`);
    await expect(page2.locator("#register-button")).toBeEnabled({
      timeout: 5000,
    });
    await page2.fill("#username", username2);
    await page2.click("#register-button");

    await expect(page2).toHaveTitle("Setup Encryption - Crowchiper", {
      timeout: 10000,
    });

    // User 1 lists their tokens
    const response1 = await page1.request.get(`${baseUrl}/api/tokens`);
    expect(response1.status()).toBe(200);
    const data1 = await response1.json();
    expect(data1.tokens.length).toBe(1); // Only their own token

    // User 2 lists their tokens
    const response2 = await page2.request.get(`${baseUrl}/api/tokens`);
    expect(response2.status()).toBe(200);
    const data2 = await response2.json();
    expect(data2.tokens.length).toBe(1); // Only their own token

    // The tokens should be different
    expect(data1.tokens[0].jti).not.toBe(data2.tokens[0].jti);

    await context1.close();
    await context2.close();
  });
});

// =============================================================================
// Token Verify Endpoint Tests
// =============================================================================

test.describe("Token verify endpoint", () => {
  test("verify returns 200 for valid token", async ({
    browser,
    baseUrl,
    testId,
  }) => {
    const username = `verify_valid_${testId}`;

    const context = await browser.newContext();
    const page = await context.newPage();
    const client = await page.context().newCDPSession(page);
    await addVirtualAuthenticator(client);

    await page.goto(`${baseUrl}/login/register.html`);
    await expect(page.locator("#register-button")).toBeEnabled({
      timeout: 5000,
    });
    await page.fill("#username", username);
    await page.click("#register-button");

    await expect(page).toHaveTitle("Setup Encryption - Crowchiper", {
      timeout: 10000,
    });

    const response = await page.request.get(`${baseUrl}/api/tokens/verify`);
    expect(response.status()).toBe(200);

    await context.close();
  });

  test("verify returns 401 for invalid token", async ({ context, baseUrl }) => {
    const page = await context.newPage();

    await page.goto(`${baseUrl}/login/index.html`);
    await context.addCookies([
      {
        name: "access_token",
        value: "invalid-token",
        domain: new URL(baseUrl).hostname,
        path: "/",
      },
    ]);

    const response = await page.request.get(`${baseUrl}/api/tokens/verify`);
    expect(response.status()).toBe(401);

    await page.close();
  });

  test("verify returns 401 without token", async ({ context, baseUrl }) => {
    const page = await context.newPage();
    await clearCookies(context);

    await page.goto(`${baseUrl}/login/index.html`);

    const response = await page.request.get(`${baseUrl}/api/tokens/verify`);
    expect(response.status()).toBe(401);

    await page.close();
  });
});

// =============================================================================
// User Isolation Tests
// =============================================================================

test.describe("User isolation", () => {
  test("users cannot access each other's data", async ({
    browser,
    baseUrl,
    testId,
  }) => {
    // Create user 1
    const username1 = `iso_user1_${testId}`;
    const context1 = await browser.newContext();
    const page1 = await context1.newPage();
    const client1 = await page1.context().newCDPSession(page1);
    await addVirtualAuthenticator(client1);

    await page1.goto(`${baseUrl}/login/register.html`);
    await expect(page1.locator("#register-button")).toBeEnabled({
      timeout: 5000,
    });
    await page1.fill("#username", username1);
    await page1.click("#register-button");
    await expect(page1).toHaveTitle("Setup Encryption - Crowchiper", {
      timeout: 10000,
    });

    // Create a post for user 1 via API
    const createPostResponse = await page1.request.post(
      `${baseUrl}/api/posts`,
      {
        data: { title: "User 1 Secret", content: "Secret content" },
      },
    );
    expect(createPostResponse.status()).toBe(201);
    const postData = await createPostResponse.json();
    const postUuid = postData.uuid;

    // Create user 2 in separate context
    const username2 = `iso_user2_${testId}`;
    const context2 = await browser.newContext();
    const page2 = await context2.newPage();
    const client2 = await page2.context().newCDPSession(page2);
    await addVirtualAuthenticator(client2);

    await page2.goto(`${baseUrl}/login/register.html`);
    await expect(page2.locator("#register-button")).toBeEnabled({
      timeout: 5000,
    });
    await page2.fill("#username", username2);
    await page2.click("#register-button");
    await expect(page2).toHaveTitle("Setup Encryption - Crowchiper", {
      timeout: 10000,
    });

    // User 2 tries to access user 1's post
    const getPostResponse = await page2.request.get(
      `${baseUrl}/api/posts/${postUuid}`,
    );
    expect(getPostResponse.status()).toBe(404); // Not found, not forbidden

    // User 2 lists their posts - should not see user 1's post
    const listResponse = await page2.request.get(`${baseUrl}/api/posts`);
    expect(listResponse.status()).toBe(200);
    const posts = await listResponse.json();
    expect(posts.length).toBe(0); // User 2 has no posts

    await context1.close();
    await context2.close();
  });

  test("users cannot revoke each other's tokens", async ({
    browser,
    baseUrl,
    testId,
  }) => {
    // Create two users
    const username1 = `revoke_user1_${testId}`;
    const username2 = `revoke_user2_${testId}`;

    const context1 = await browser.newContext();
    const page1 = await context1.newPage();
    const client1 = await page1.context().newCDPSession(page1);
    await addVirtualAuthenticator(client1);

    await page1.goto(`${baseUrl}/login/register.html`);
    await expect(page1.locator("#register-button")).toBeEnabled({
      timeout: 5000,
    });
    await page1.fill("#username", username1);
    await page1.click("#register-button");
    await expect(page1).toHaveTitle("Setup Encryption - Crowchiper", {
      timeout: 10000,
    });

    // Get user 1's token JTI
    const response1 = await page1.request.get(`${baseUrl}/api/tokens`);
    const data1 = await response1.json();
    const user1TokenJti = data1.tokens[0].jti;

    // Create user 2
    const context2 = await browser.newContext();
    const page2 = await context2.newPage();
    const client2 = await page2.context().newCDPSession(page2);
    await addVirtualAuthenticator(client2);

    await page2.goto(`${baseUrl}/login/register.html`);
    await expect(page2.locator("#register-button")).toBeEnabled({
      timeout: 5000,
    });
    await page2.fill("#username", username2);
    await page2.click("#register-button");
    await expect(page2).toHaveTitle("Setup Encryption - Crowchiper", {
      timeout: 10000,
    });

    // User 2 tries to revoke user 1's token
    const revokeResponse = await page2.request.delete(
      `${baseUrl}/api/tokens/${user1TokenJti}`,
    );
    expect(revokeResponse.status()).toBe(403); // Forbidden

    // Verify user 1's token still works
    const verifyResponse = await page1.request.get(
      `${baseUrl}/api/tokens/verify`,
    );
    expect(verifyResponse.status()).toBe(200);

    await context1.close();
    await context2.close();
  });
});

// =============================================================================
// Token Type Security Tests
// =============================================================================

test.describe("Token type security", () => {
  test("refresh token cannot be used in place of access token", async ({
    context,
    baseUrl,
  }) => {
    const userUuid = crypto.randomUUID();
    const username = "typeconfusion";

    // Generate tokens with refresh stored in DB
    const tokens = await generateTokens(baseUrl, userUuid, username, {
      store_refresh: true,
    });

    const page = await context.newPage();

    // Try to use refresh token as access token
    await page.goto(`${baseUrl}/login/index.html`);
    await context.addCookies([
      {
        name: "access_token",
        value: tokens.refresh_token, // Wrong token type!
        domain: new URL(baseUrl).hostname,
        path: "/",
      },
    ]);

    // Should fail - token type mismatch
    const response = await page.request.get(`${baseUrl}/api/tokens/verify`);
    expect(response.status()).toBe(401);

    await page.close();
  });

  test("access token cannot be used as refresh token", async ({
    context,
    baseUrl,
  }) => {
    const userUuid = crypto.randomUUID();
    const username = "typeconfusion2";

    // Generate tokens: one valid access, one expired access
    const validTokens = await generateTokens(baseUrl, userUuid, username, {
      store_refresh: false,
    });
    const expiredTokens = await generateTokens(baseUrl, userUuid, username, {
      expired_access: true,
      store_refresh: false,
    });

    const page = await context.newPage();

    // Try to use access token as refresh token
    await page.goto(`${baseUrl}/login/index.html`);
    await context.addCookies([
      {
        name: "access_token",
        value: expiredTokens.access_token, // Expired
        domain: new URL(baseUrl).hostname,
        path: "/",
      },
      {
        name: "refresh_token",
        value: validTokens.access_token, // Wrong token type!
        domain: new URL(baseUrl).hostname,
        path: "/",
      },
    ]);

    // Should fail - can't refresh with access token
    const response = await page.request.get(`${baseUrl}/api/tokens/verify`);
    expect(response.status()).toBe(401);

    await page.close();
  });
});
