import {
  test,
  expect,
  createUser,
  uniqueTestId,
  CreateUserResult,
} from "../utils/fixtures.ts";
import { getServer, Server } from "../utils/server.ts";
import { BrowserContext } from "@playwright/test";

test.describe("Sidebar collapse functionality", () => {
  let context: BrowserContext;
  let userResult: CreateUserResult;

  test.beforeAll(async ({ browser }) => {
    const { baseUrl } = await getServer(Server.Default);
    context = await browser.newContext();
    const username = `sidebar_${uniqueTestId()}`;
    userResult = await createUser({
      context,
      baseUrl,
      username,
      enableEncryption: true,
    });
  });

  test.afterAll(async () => {
    await context?.close();
  });

  test("sidebar toggle collapses on small screen", async () => {
    const { page } = userResult;

    // Set viewport to mobile size (triggers responsive behavior at <= 768px)
    await page.setViewportSize({ width: 600, height: 800 });

    // Wait for sidebar to be present
    const sidebar = page.locator("#sidebar");
    await expect(sidebar).toBeVisible({ timeout: 5000 });

    // Verify toggle button is visible on small screen
    const toggleBtn = page.locator("#sidebar-toggle");
    await expect(toggleBtn).toBeVisible({ timeout: 5000 });

    // Sidebar should not be collapsed initially
    await expect(sidebar).not.toHaveAttribute("data-collapsed");

    // Verify aria-expanded is initially true
    await expect(toggleBtn).toHaveAttribute("aria-expanded", "true");

    // Click the toggle button to collapse
    await toggleBtn.click();

    // Verify sidebar is now collapsed
    await expect(sidebar).toHaveAttribute("data-collapsed");

    // Verify aria-expanded is now false
    await expect(toggleBtn).toHaveAttribute("aria-expanded", "false");

    // Click again to expand
    await toggleBtn.click();

    // Verify sidebar is expanded again
    await expect(sidebar).not.toHaveAttribute("data-collapsed");

    // Verify aria-expanded is true again
    await expect(toggleBtn).toHaveAttribute("aria-expanded", "true");
  });

  test("sidebar toggle hidden on large screen", async () => {
    const { page } = userResult;

    // Set viewport to desktop size
    await page.setViewportSize({ width: 1200, height: 800 });

    // Wait for sidebar to be present
    const sidebar = page.locator("#sidebar");
    await expect(sidebar).toBeVisible({ timeout: 5000 });

    // Verify toggle button is hidden on large screen
    const toggleBtn = page.locator("#sidebar-toggle");
    await expect(toggleBtn).toBeHidden({ timeout: 5000 });
  });
});
