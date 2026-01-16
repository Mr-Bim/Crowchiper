import { test, expect } from "../utils/fixtures.ts";

test.describe("Theme toggle", () => {
  test("changes theme when selecting different option", async ({
    page,
    baseUrl,
  }) => {
    await page.goto(`${baseUrl}/login/index.html`);

    // Wait for theme select to be created by inline.ts
    const themeSelect = page.locator("#theme-select");
    await expect(themeSelect).toBeVisible({ timeout: 5000 });

    // Get initial theme
    const initialTheme = await page.evaluate(() =>
      document.documentElement.getAttribute("data-theme"),
    );

    // Select a different theme
    const newTheme =
      initialTheme === "warm-light" ? "scandi-dark" : "warm-light";
    await themeSelect.selectOption(newTheme);

    // Verify theme changed
    await expect(page.locator("html")).toHaveAttribute("data-theme", newTheme);
  });
});
