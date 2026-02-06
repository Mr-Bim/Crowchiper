import { test, expect } from "../utils/fixtures.ts";

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
      store_refresh: options.store_refresh ?? false,
    }),
  });
  if (!response.ok) {
    throw new Error(`Failed to generate tokens: ${response.status}`);
  }
  return response.json();
}

test.describe("Admin dashboard access", () => {
  test("dashboard not accessible for non-admin", async ({
    page,
    baseUrl,
    testId,
  }) => {
    const userUuid = crypto.randomUUID();

    const tokens = await generateTokens(
      baseUrl,
      userUuid,
      `regular_${testId}`,
      {
        role: "user",
        store_refresh: true,
      },
    );

    // Set cookies
    await page.goto(`${baseUrl}/login/index.html`);
    await page.evaluate(
      ({ access, refresh }) => {
        document.cookie = `access_token=${access}; path=/`;
        document.cookie = `refresh_token=${refresh}; path=/`;
      },
      { access: tokens.access_token, refresh: tokens.refresh_token },
    );

    // Try to access dashboard
    await page.goto(`${baseUrl}/dashboard/`);

    // Non-admin gets redirected away from dashboard
    // (to login, which may then redirect to app since user is authenticated)
    await expect(page).not.toHaveURL(/\/dashboard/, { timeout: 5000 });
  });

  test("dashboard accessible for admin", async ({ page, baseUrl, testId }) => {
    const adminUuid = crypto.randomUUID();

    const tokens = await generateTokens(baseUrl, adminUuid, `admin_${testId}`, {
      role: "admin",
      store_refresh: true,
    });

    // Set cookies
    await page.goto(`${baseUrl}/login/index.html`);
    await page.evaluate(
      ({ access, refresh }) => {
        document.cookie = `access_token=${access}; path=/`;
        document.cookie = `refresh_token=${refresh}; path=/`;
      },
      { access: tokens.access_token, refresh: tokens.refresh_token },
    );

    // Navigate to dashboard
    await page.goto(`${baseUrl}/dashboard/`);

    // Should stay on dashboard
    await expect(page).toHaveURL(/\/dashboard/, { timeout: 5000 });
    await expect(page).toHaveTitle("Dashboard - Crowchiper");
  });

  test("dashboard redirects without auth", async ({ page, baseUrl }) => {
    // Clear cookies
    await page.goto(`${baseUrl}/login/index.html`);
    await page.evaluate(
      "document.cookie = 'access_token=; path=/; expires=Thu, 01 Jan 1970 00:00:00 GMT'",
    );
    await page.evaluate(
      "document.cookie = 'refresh_token=; path=/; expires=Thu, 01 Jan 1970 00:00:00 GMT'",
    );

    await page.goto(`${baseUrl}/dashboard/`);

    // Should redirect to login
    await expect(page).toHaveURL(/\/login/, { timeout: 5000 });
  });
});

test.describe("Admin API access control", () => {
  test("admin users API returns 401 without auth", async ({ baseUrl }) => {
    const response = await fetch(`${baseUrl}/api/admin/users`);
    expect(response.status).toBe(401);
  });

  test("admin users API returns 403 for non-admin", async ({
    baseUrl,
    testId,
  }) => {
    const userUuid = crypto.randomUUID();

    const tokens = await generateTokens(baseUrl, userUuid, `apireg_${testId}`, {
      role: "user",
      store_refresh: true,
    });

    const response = await fetch(`${baseUrl}/api/admin/users`, {
      headers: {
        Cookie: `access_token=${tokens.access_token}; refresh_token=${tokens.refresh_token}`,
      },
    });
    expect(response.status).toBe(403);
  });

  test("admin users API returns 200 for admin", async ({ baseUrl, testId }) => {
    const adminUuid = crypto.randomUUID();

    const tokens = await generateTokens(
      baseUrl,
      adminUuid,
      `apiadm_${testId}`,
      {
        role: "admin",
        store_refresh: true,
      },
    );

    const response = await fetch(`${baseUrl}/api/admin/users`, {
      headers: {
        Cookie: `access_token=${tokens.access_token}; refresh_token=${tokens.refresh_token}`,
      },
    });
    expect(response.status).toBe(200);

    const users = await response.json();
    expect(Array.isArray(users)).toBe(true);
  });
});

test.describe("Dashboard users table", () => {
  test("users table renders for admin", async ({ page, baseUrl, testId }) => {
    const adminUuid = crypto.randomUUID();

    const tokens = await generateTokens(
      baseUrl,
      adminUuid,
      `tableadmin_${testId}`,
      {
        role: "admin",
        store_refresh: true,
      },
    );

    // Set cookies
    await page.goto(`${baseUrl}/login/index.html`);
    await page.evaluate(
      ({ access, refresh }) => {
        document.cookie = `access_token=${access}; path=/`;
        document.cookie = `refresh_token=${refresh}; path=/`;
      },
      { access: tokens.access_token, refresh: tokens.refresh_token },
    );

    // Navigate to dashboard
    await page.goto(`${baseUrl}/dashboard/`);

    // Wait for users table to appear
    const table = page.locator('[data-testid="test-users-table"]');
    await expect(table).toBeVisible({ timeout: 10000 });

    // Table should have at least one row
    const rows = table.locator("tbody tr");
    const count = await rows.count();
    expect(count).toBeGreaterThanOrEqual(1);
  });
});
