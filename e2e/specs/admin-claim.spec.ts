import {
  test,
  expect,
  APP_PATH,
  uniqueTestId,
  addVirtualAuthenticator,
  generateTestPrfOutput,
  injectTestPrfOutput,
  injectTestUsername,
} from "../utils/fixtures.ts";
import { getServer, Server } from "../utils/server.ts";
import { BrowserContext } from "@playwright/test";

/** Create an admin user via the test API endpoint */
async function createAdmin(
  baseUrl: string,
  username: string,
): Promise<{ uuid: string; username: string }> {
  const response = await fetch(`${baseUrl}/api/test/admin`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ username }),
  });
  if (!response.ok) {
    throw new Error(`Failed to create admin: ${response.status}`);
  }
  return response.json();
}

test.describe("Admin claim with encryption setup", () => {
  let context: BrowserContext;
  let baseUrl: string;

  test.beforeAll(async ({ browser }) => {
    const server = await getServer(Server.Default);
    baseUrl = server.baseUrl;
    context = await browser.newContext();
  });

  test.afterAll(async () => {
    await context?.close();
  });

  test("admin claim redirects to encryption setup and completes full flow", async () => {
    const page = await context.newPage();
    const cdpSession = await page.context().newCDPSession(page);
    await addVirtualAuthenticator(cdpSession);

    const adminUsername = `claimadmin_${uniqueTestId()}`;
    const admin = await createAdmin(baseUrl, adminUsername);

    // Inject test PRF output and username for encryption setup
    await injectTestPrfOutput(page, generateTestPrfOutput());
    await injectTestUsername(page, adminUsername);

    // Navigate to claim page with admin UUID
    await page.goto(`${baseUrl}/login/claim.html?uuid=${admin.uuid}`);

    // Wait for claim button to be enabled
    const claimButton = page.locator("#claim-button");
    await expect(claimButton).toBeEnabled({ timeout: 5000 });
    await expect(claimButton).toHaveText("Register Passkey");

    // Click to register passkey
    await claimButton.click();

    // Should redirect to setup-encryption page (not login)
    await page.waitForURL(new RegExp(`${APP_PATH}/setup-encryption.html`), {
      timeout: 10000,
    });

    // Test PRF support
    const testPrfBtn = page.locator("#test-prf-btn");
    await testPrfBtn.waitFor({ state: "visible" });
    await testPrfBtn.click();

    // Enable encryption
    const enableEncryptionBtn = page.locator("#enable-encryption-btn");
    await enableEncryptionBtn.waitFor({ state: "visible", timeout: 10000 });
    await enableEncryptionBtn.click();

    // Should redirect to app
    await page.waitForURL(new RegExp(`${APP_PATH}(/index\\.html)?$`), {
      timeout: 10000,
    });

    // Unlock with passkey
    const unlockBtn = page.locator("#unlock-btn");
    await unlockBtn.waitFor({ state: "visible" });
    await unlockBtn.click();

    // Unlock overlay should disappear
    const unlockOverlay = page.locator("#unlock-overlay");
    await unlockOverlay.waitFor({ state: "hidden", timeout: 10000 });
  });
});
