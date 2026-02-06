import { test, expect, addVirtualAuthenticator } from "../utils/fixtures.ts";

test.describe("Claim page", () => {
  test("page loads with correct title", async ({ page, baseUrl }) => {
    // Navigate with a fake UUID to get into register mode
    const fakeUuid = "12345678-1234-1234-1234-123456789012";
    await page.goto(`${baseUrl}/login/claim.html?uuid=${fakeUuid}`);

    await expect(page).toHaveTitle("Claim Account - Crowchiper");
  });

  test("shows error without uuid parameter", async ({ page, baseUrl }) => {
    await page.goto(`${baseUrl}/login/claim.html`);

    // Wait for JS to initialize
    await page.waitForTimeout(200);

    // Check status shows invalid link message
    const status = page.locator("#status");
    await expect(status).toContainText("Invalid");

    // Button should remain disabled
    const button = page.locator("#claim-button");
    await expect(button).toBeDisabled();
  });

  test("claim admin user success", async ({ context, baseUrl, testId }) => {
    // Create an admin user via test API
    const username = `admin_${testId}`;
    const response = await context.request.post(`${baseUrl}/api/test/admin`, {
      data: { username },
    });
    expect(response.ok()).toBe(true);
    const { uuid } = await response.json();

    // Create a new page with virtual authenticator
    const page = await context.newPage();
    const client = await page.context().newCDPSession(page);
    await addVirtualAuthenticator(client);

    // Navigate to claim page with UUID
    await page.goto(`${baseUrl}/login/claim.html?uuid=${uuid}`);

    // Wait for button to be enabled
    const claimButton = page.locator("#claim-button");
    await expect(claimButton).toBeEnabled({ timeout: 5000 });

    // Check status text shows instruction
    const status = page.locator("#status");
    await expect(status).toContainText("passkey");

    // Click the claim button
    await claimButton.click();

    // Wait for redirect to encryption setup page after successful claim
    await expect(page).toHaveTitle("Setup Encryption - Crowchiper", {
      timeout: 10000,
    });

    await page.close();
  });

  test("claim with invalid uuid fails", async ({ context, baseUrl }) => {
    // Create a new page with virtual authenticator
    const page = await context.newPage();
    const client = await page.context().newCDPSession(page);
    await addVirtualAuthenticator(client);

    // Navigate to claim page with a non-existent UUID
    const fakeUuid = "00000000-0000-0000-0000-000000000000";
    await page.goto(`${baseUrl}/login/claim.html?uuid=${fakeUuid}`);

    // Wait for button to be enabled
    const claimButton = page.locator("#claim-button");
    await expect(claimButton).toBeEnabled({ timeout: 5000 });

    // Click the claim button
    await claimButton.click();

    // Wait for error message to appear in status
    const status = page.locator("#status");
    await expect(status).toContainText(/not found|Failed/i, { timeout: 5000 });

    await page.close();
  });
});
