import { BrowserContext } from "@playwright/test";
import {
  test,
  expect,
  createUser,
  uniqueTestId,
  CreateUserResult,
  appUrl,
  APP_PATH,
} from "../utils/fixtures.ts";
import { getServer, Server } from "../utils/server.ts";

test.describe("Logout functionality", () => {
  let context: BrowserContext;
  let userResult: CreateUserResult;
  let username: string;
  let baseUrl: string;

  test.beforeAll(async ({ browser }) => {
    const server = await getServer(Server.Default);
    baseUrl = server.baseUrl;

    context = await browser.newContext();

    username = `logout_${uniqueTestId()}`;
    userResult = await createUser({
      context,
      baseUrl,
      username,
    });
  });

  test.afterAll(async () => {
    await context?.close();
  });

  test("logout clears token and redirects to login", async () => {
    const { page } = userResult;

    // Verify we're on the app page
    await expect(page).toHaveURL(new RegExp(APP_PATH));

    // Verify token is valid before logout
    const verifyBeforeLogout = await page.evaluate(async (apiPath) => {
      const response = await fetch(`${apiPath}/tokens/verify`, {
        credentials: "include",
      });
      return response.ok;
    }, `${baseUrl}/api`);
    expect(verifyBeforeLogout).toBe(true);

    // Open the settings menu
    const settingsButton = page.locator("#settings-btn");
    await settingsButton.click();

    const logoutButton = page.locator("#logout-btn");
    await logoutButton.click();

    // Should be on login page
    await expect(page).toHaveURL(/\/login/);

    // Verify token is now invalid (cookie cleared)
    const verifyAfterLogout = await page.evaluate(async (apiPath) => {
      const response = await fetch(`${apiPath}/tokens/verify`, {
        credentials: "include",
      });
      return response.ok;
    }, `${baseUrl}/api`);
    expect(verifyAfterLogout).toBe(false);

    // Try to navigate back to the app - should redirect to login
    await page.goto(`${appUrl(baseUrl)}/index.html`);
    await expect(page).toHaveURL(/\/login/, { timeout: 5000 });
  });
});
