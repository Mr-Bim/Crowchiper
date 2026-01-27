/**
 * Settings Panel E2E Tests
 *
 * Tests for the user settings panel:
 * - Opening/closing the panel
 * - Viewing active sessions
 * - Revoking sessions
 */

import { BrowserContext } from "@playwright/test";
import {
  test,
  expect,
  createUser,
  uniqueTestId,
  CreateUserResult,
  APP_PATH,
  addVirtualAuthenticator,
} from "../utils/fixtures.ts";
import { getServer, Server } from "../utils/server.ts";

test.describe("Settings panel", () => {
  let context: BrowserContext;
  let userResult: CreateUserResult;
  let username: string;
  let baseUrl: string;

  test.beforeAll(async ({ browser }) => {
    const server = await getServer(Server.Default);
    baseUrl = server.baseUrl;

    context = await browser.newContext();

    username = `settings_${uniqueTestId()}`;
    userResult = await createUser({
      context,
      baseUrl,
      username,
    });
  });

  test.afterAll(async () => {
    await context?.close();
  });

  test("can open and close settings panel", async () => {
    const { page } = userResult;

    // Verify we're on the app page
    await expect(page).toHaveURL(new RegExp(APP_PATH));

    // Open the settings menu
    const settingsButton = page.locator("#settings-btn");
    await settingsButton.click();

    // Click manage sessions
    const manageSessionsButton = page.locator("#manage-sessions-btn");
    await manageSessionsButton.click();

    // Settings panel should be visible
    const settingsPanel = page.locator("#settings-panel");
    await expect(settingsPanel).toBeVisible();

    // Close with the X button
    const closeButton = page.locator("#settings-panel-close");
    await closeButton.click();

    // Panel should be hidden
    await expect(settingsPanel).toBeHidden();
  });

  test("can close settings panel with Escape key", async () => {
    const { page } = userResult;

    // Open settings panel
    await page.locator("#settings-btn").click();
    await page.locator("#manage-sessions-btn").click();

    const settingsPanel = page.locator("#settings-panel");
    await expect(settingsPanel).toBeVisible();

    // Press Escape
    await page.keyboard.press("Escape");

    // Panel should be hidden
    await expect(settingsPanel).toBeHidden();
  });

  test("shows current session in sessions list", async () => {
    const { page } = userResult;

    // Open settings panel
    await page.locator("#settings-btn").click();
    await page.locator("#manage-sessions-btn").click();

    const settingsPanel = page.locator("#settings-panel");
    await expect(settingsPanel).toBeVisible();

    // Wait for sessions to load
    const sessionsList = page.locator("#sessions-list");
    await expect(
      sessionsList.locator('[data-testid="test-session-item"]'),
    ).toHaveCount(1, {
      timeout: 5000,
    });

    // Should show "Current" badge
    const currentBadge = sessionsList.locator(
      '[data-testid="test-session-current"]',
    );
    await expect(currentBadge).toBeVisible();
    await expect(currentBadge).toHaveText("Current");

    // Current session should not have revoke button
    const revokeButtons = sessionsList.locator(
      '[data-testid="test-session-revoke"]',
    );
    await expect(revokeButtons).toHaveCount(0);

    // Close panel
    await page.keyboard.press("Escape");
  });
});

test.describe("Session revocation via settings panel", () => {
  test("can revoke other sessions", async ({ browser }) => {
    const server = await getServer(Server.Default);
    const baseUrl = server.baseUrl;
    const username = `revoke_${uniqueTestId()}`;

    // Create first session
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

    // Get the first session's refresh token
    const cookies1 = await context1.cookies();
    const token1 = cookies1.find((c) => c.name === "refresh_token")?.value;
    expect(token1).toBeDefined();

    // Clear cookies and create second session (simulating another device)
    await client1.send("Network.clearBrowserCookies");

    await page1.goto(`${baseUrl}/login/index.html`);
    await expect(page1.locator("#login-button")).toBeEnabled({ timeout: 5000 });
    await page1.fill("#username", username);
    await page1.click("#login-button");

    await expect(page1).toHaveURL(
      new RegExp(`${APP_PATH}/setup-encryption.html`),
      { timeout: 10000 },
    );

    // Verify we now have 2 sessions
    const tokensResponse = await page1.request.get(`${baseUrl}/api/tokens`);
    const tokensData = await tokensResponse.json();
    expect(tokensData.tokens.length).toBe(2);

    // Open settings panel
    await page1.goto(`${baseUrl}${APP_PATH}/index.html`);
    await page1.locator("#settings-btn").click();
    await page1.locator("#manage-sessions-btn").click();

    const settingsPanel = page1.locator("#settings-panel");
    await expect(settingsPanel).toBeVisible();

    // Wait for sessions to load - should see 2
    const sessionsList = page1.locator("#sessions-list");
    await expect(
      sessionsList.locator('[data-testid="test-session-item"]'),
    ).toHaveCount(2, {
      timeout: 5000,
    });

    // Find the revoke button (only the non-current session has one)
    const revokeButton = sessionsList.locator(
      '[data-testid="test-session-revoke"]',
    );
    await expect(revokeButton).toHaveCount(1);

    // Revoke the other session and wait for it to disappear
    await revokeButton.click();

    // Wait for the button to disappear (indicates revocation completed)
    await expect(revokeButton).toHaveCount(0, { timeout: 5000 });

    // Should now only have 1 session
    await expect(
      sessionsList.locator('[data-testid="test-session-item"]'),
    ).toHaveCount(1);

    // Verify via API
    const tokensResponse2 = await page1.request.get(`${baseUrl}/api/tokens`);
    const tokensData2 = await tokensResponse2.json();
    expect(tokensData2.tokens.length).toBe(1);

    await context1.close();
  });

  test("revoked session cannot access app", async ({ browser }) => {
    const server = await getServer(Server.Default);
    const baseUrl = server.baseUrl;
    const username = `revoke_access_${uniqueTestId()}`;

    // Create user with first session
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

    // Save the first session's cookies
    const cookies1 = await context1.cookies();
    const refreshToken1 = cookies1.find(
      (c) => c.name === "refresh_token",
    )?.value;
    expect(refreshToken1).toBeDefined();

    // Clear cookies and create second session
    await client1.send("Network.clearBrowserCookies");

    await page1.goto(`${baseUrl}/login/index.html`);
    await expect(page1.locator("#login-button")).toBeEnabled({ timeout: 5000 });
    await page1.fill("#username", username);
    await page1.click("#login-button");

    await expect(page1).toHaveURL(
      new RegExp(`${APP_PATH}/setup-encryption.html`),
      { timeout: 10000 },
    );

    // From session 2, revoke session 1 via settings panel
    await page1.goto(`${baseUrl}${APP_PATH}/index.html`);
    await page1.locator("#settings-btn").click();
    await page1.locator("#manage-sessions-btn").click();

    const sessionsList = page1.locator("#sessions-list");
    await expect(
      sessionsList.locator('[data-testid="test-session-item"]'),
    ).toHaveCount(2, {
      timeout: 5000,
    });

    const revokeButton = sessionsList.locator(
      '[data-testid="test-session-revoke"]',
    );
    await revokeButton.click();

    // Wait for the button to disappear (indicates revocation completed)
    await expect(revokeButton).toHaveCount(0, { timeout: 5000 });

    await expect(
      sessionsList.locator('[data-testid="test-session-item"]'),
    ).toHaveCount(1);

    // Now try to use session 1's token in a new context
    const context2 = await browser.newContext();
    const page2 = await context2.newPage();

    await page2.goto(`${baseUrl}/login/index.html`);
    await context2.addCookies([
      {
        name: "refresh_token",
        value: refreshToken1!,
        domain: new URL(baseUrl).hostname,
        path: "/",
      },
    ]);

    // Try to access app - should fail and stay on login
    await page2.goto(`${baseUrl}${APP_PATH}/index.html`);

    // Should redirect to login since token is revoked
    await expect(page2).toHaveURL(/\/login/, { timeout: 5000 });

    await context1.close();
    await context2.close();
  });
});
