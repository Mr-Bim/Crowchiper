import { test, expect, Server } from "../utils/fixtures.ts";

test.describe("Reclaim page", () => {
  test("claim page loads in reclaim mode", async ({ page, baseUrl }) => {
    await page.goto(`${baseUrl}/login/claim.html?reclaim=true`);

    // Wait for JS to initialize
    await page.waitForTimeout(200);

    // Check title is set for reclaim mode
    const title = page.locator("#claim-title");
    await expect(title).toHaveText("Reclaim Account");

    // Check status message mentions reclaiming or passkey
    const status = page.locator("#status");
    const statusText = await status.textContent();
    expect(statusText).toMatch(/reclaim|passkey/i);

    // Button should say "Use Passkey"
    const button = page.locator("#claim-button");
    await expect(button).toHaveText("Use Passkey");

    // Button should be enabled in reclaim mode
    await expect(button).toBeEnabled();
  });

  test("claim page loads in register mode with uuid", async ({
    page,
    baseUrl,
  }) => {
    // Use a fake UUID - the page should still load even if the UUID doesn't exist
    // (it will show an error when trying to claim, but page should render)
    const fakeUuid = "12345678-1234-1234-1234-123456789012";
    await page.goto(`${baseUrl}/login/claim.html?uuid=${fakeUuid}`);

    // Wait for JS to initialize
    await page.waitForTimeout(200);

    // Check title is set for register mode
    const title = page.locator("#claim-title");
    await expect(title).toHaveText("Claim Admin");

    // Button should say "Register Passkey"
    const button = page.locator("#claim-button");
    await expect(button).toHaveText("Register Passkey");
  });

  test("claim page loads in register mode with admin user", async ({
    context,
    baseUrl,
    testId,
  }) => {
    // Create a pending admin user via test API
    const username = `reclaimadmin_${testId}`;
    const response = await context.request.post(`${baseUrl}/api/test/admin`, {
      data: { username },
    });
    expect(response.ok()).toBe(true);
    const { uuid } = await response.json();

    const page = await context.newPage();
    await page.goto(`${baseUrl}/login/claim.html?uuid=${uuid}`);

    // Wait for JS to initialize
    await page.waitForTimeout(200);

    // Check title is set for register mode
    const title = page.locator("#claim-title");
    await expect(title).toHaveText("Claim Admin");

    // Button should say "Register Passkey"
    const button = page.locator("#claim-button");
    await expect(button).toHaveText("Register Passkey");

    await page.close();
  });
});

test.describe("Reclaim API", () => {
  test("claim/start returns webauthn options", async ({ context, baseUrl }) => {
    const response = await context.request.post(
      `${baseUrl}/api/passkeys/claim/start`,
    );

    expect(response.ok()).toBe(true);
    const json = await response.json();

    // Should have a session_id
    expect(json.session_id).toBeDefined();
    expect(typeof json.session_id).toBe("string");

    // Should have publicKey with challenge
    expect(json.publicKey).toBeDefined();
    expect(json.publicKey.challenge).toBeDefined();
    expect(typeof json.publicKey.challenge).toBe("string");

    // Should have rpId
    expect(json.publicKey.rpId).toBe("localhost");
  });

  test("claim/start works with base path", async ({
    context,
    getServerUrl,
  }) => {
    const baseUrl = await getServerUrl(Server.BasePath);

    const response = await context.request.post(
      `${baseUrl}/api/passkeys/claim/start`,
    );

    expect(response.ok()).toBe(true);
    const json = await response.json();

    // Should have a session_id
    expect(json.session_id).toBeDefined();
    expect(json.publicKey).toBeDefined();
  });

  test("login/start works for unactivated user", async ({
    context,
    baseUrl,
    testId,
  }) => {
    // Create a regular (non-admin) user via the public API
    // This creates an unactivated user
    const username = `unactivated_${testId}`;
    const createResponse = await context.request.post(`${baseUrl}/api/users`, {
      data: { username },
    });
    expect(createResponse.ok()).toBe(true);

    // login/start should work for unactivated user
    const loginResponse = await context.request.post(
      `${baseUrl}/api/passkeys/login/start`,
      {
        data: { username },
      },
    );

    expect(loginResponse.ok()).toBe(true);
    const json = await loginResponse.json();

    // Should return valid WebAuthn options
    expect(json.session_id).toBeDefined();
    expect(json.publicKey).toBeDefined();
  });
});
