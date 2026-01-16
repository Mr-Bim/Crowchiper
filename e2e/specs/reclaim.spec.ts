import { test, expect } from "../utils/fixtures.ts";

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
});
