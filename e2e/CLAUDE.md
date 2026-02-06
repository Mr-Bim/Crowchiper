# E2E Tests

Playwright tests with Chrome. Tests run in parallel across files.

## Commands

```bash
npx playwright test        # Run all e2e tests
npx playwright test --ui   # Run with UI mode
```

## Environment Variables

- `CROWCHIPER_BIN` - Path to the crowchiper binary. Defaults to `target/debug/crowchiper`.

## Important: Use .ts Extensions

All relative imports MUST include the `.ts` extension:

```typescript
// Correct
import { test, expect, Server } from "./fixtures.ts";

// Wrong - will fail
import { test, expect } from "./fixtures";
```

## Files

- `fixtures.ts` - Test fixtures and utilities
- `server.ts` - Server manager with `Server` enum
- `global-setup.ts` - Pre-starts all server configurations
- `global-teardown.ts` - Stops all servers

## Writing Parallel-Safe Tests

Tests across different files run in parallel. To avoid conflicts:

### 1. Always use `testId` for usernames

```typescript
import { test, expect } from "./fixtures.ts";

test("register user", async ({ page, baseUrl, testId }) => {
  const username = `mytest_${testId}`;
  await page.fill("#username", username);
  await page.click("#register-button");
});
```

### 2. Use the `Server` enum for different server configs

```typescript
import { test, expect, Server } from "./fixtures.ts";

// Default server
test("basic test", async ({ page, baseUrl }) => {
  await page.goto(`${baseUrl}/login/`);
});

// Server with signup disabled
test("no-signup test", async ({ page, getServerUrl }) => {
  const baseUrl = await getServerUrl(Server.NoSignup);
  await page.goto(`${baseUrl}/login/`);
});

// Server with base path (/crow-chipher)
test("base path test", async ({ page, getServerUrl }) => {
  const baseUrl = await getServerUrl(Server.BasePath);
  await page.goto(`${baseUrl}/login/`);
});
```

## Available Fixtures

- `baseUrl` - Default server URL
- `getServerUrl(server: Server)` - Get URL for any server configuration
- `testId` - Unique ID for test isolation (use for usernames)
- `page` - Playwright page with WebAuthn enabled
- `context` - Browser context
- `cdpSession` - CDP session for advanced WebAuthn control

## Creating Users in beforeAll

Use `createUser()` to set up a user once for all tests in a file. This avoids repeating registration/encryption setup in each test.

```typescript
import {
  test,
  expect,
  createUser,
  uniqueTestId,
  CreateUserResult,
} from "./fixtures.ts";
import { getServer, Server } from "./server.ts";
import { BrowserContext, Page } from "@playwright/test";

test.describe("My feature tests", () => {
  let context: BrowserContext;
  let userResult: CreateUserResult;
  let username: string;

  test.beforeAll(async ({ browser }) => {
    // Get server URL
    const { baseUrl } = await getServer(Server.Default);

    // Create a shared context
    context = await browser.newContext();

    // Create user with encryption enabled
    username = `myfeature_${uniqueTestId()}`;
    userResult = await createUser({
      context,
      baseUrl,
      username,
      enableEncryption: true, // default, can omit
    });
  });

  test.afterAll(async () => {
    await context?.close();
  });

  test("first test", async () => {
    const { page } = userResult;
    // page is already logged in and unlocked
    await expect(page.locator("#editor")).toBeVisible();
  });

  test("second test", async () => {
    const { page } = userResult;
    // same user, same session
  });
});
```

### createUser Options

- `context` - Browser context (create via `browser.newContext()`)
- `baseUrl` - Server base URL (get via `getServer(Server.Default)`)
- `username` - Unique username (use `uniqueTestId()` for isolation)
- `enableEncryption` - Whether to enable encryption (default: `true`)

### createUser Returns

- `page` - Logged-in page (at app index, unlocked if encryption enabled)
- `cdpSession` - CDP session for the page
- `prfOutput` - The PRF output used (null if encryption disabled)

## Available Server Configurations

Defined in `Server` enum:
- `Server.Default` - Standard server
- `Server.NoSignup` - Server with `--no-signup` flag
- `Server.BasePath` - Server with `--base /crow-chipher`

## Adding a New Server Configuration

1. Add to `Server` enum in `server.ts`
2. Add config to `SERVER_CONFIGS` object
3. It will be auto-started in global setup

## Test Files

- **`register.spec.ts`** - User registration flow tests
- **`login.spec.ts`** - Login flow and navigation tests
- **`encryption.spec.ts`** - Encryption setup and usage tests
- **`app-auth.spec.ts`** - Authentication, JWT, and authorization tests
- **`admin-claim.spec.ts`** - Admin claim flow with encryption setup
- **`admin-dashboard.spec.ts`** - Admin dashboard access control and users table tests
- **`tokens.spec.ts`** - Comprehensive token system tests (see below)
- **`post-navigation.spec.ts`** - Post switching, save behavior, editor state tests
- **`reorder.spec.ts`** - Drag and drop post reordering tests
- **`nested-posts/*.spec.ts`** - Hierarchical post structure tests

### Token Tests (`tokens.spec.ts`)

Comprehensive E2E tests for the dual-token authentication system:

**Token Issuance:**
- Registration issues refresh token cookie
- Login issues new refresh token when none exists
- Login reuses existing valid refresh token (redirects to app)

**Token Refresh Flow:**
- Expired access token triggers automatic refresh via refresh token
- Missing refresh token returns 401
- Revoked refresh token returns 401

**Multiple Sessions:**
- User can have multiple active sessions
- Logging out one session doesn't affect others

**Logout:**
- Logout clears both cookies
- Logout revokes refresh token in database
- Logout succeeds even without valid token

**Token List API:**
- Users can only see their own tokens

**User Isolation:**
- Users cannot access each other's data
- Users cannot revoke each other's tokens

**Token Type Security:**
- Refresh token cannot be used as access token
- Access token cannot be used as refresh token
