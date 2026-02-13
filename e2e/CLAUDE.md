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
import { test, expect, Server } from "../utils/fixtures.ts";

// Wrong - will fail
import { test, expect } from "../utils/fixtures";
```

## Directory Structure

```
e2e/
├── utils/
│   ├── fixtures.ts      # Test fixtures and utilities
│   └── server.ts        # Server manager with Server enum
├── setup/
│   ├── global-setup.ts  # Pre-starts all server configurations
│   └── global-teardown.ts # Stops all servers
├── specs/               # Test spec files
│   └── nested-posts/    # Nested post test suite with shared helpers
└── assets/              # Test assets (images, etc.)
```

## Writing Parallel-Safe Tests

Tests across different files run in parallel. To avoid conflicts:

### 1. Always use `testId` for usernames

```typescript
import { test, expect } from "../utils/fixtures.ts";

test("register user", async ({ page, baseUrl, testId }) => {
  const username = `mytest_${testId}`;
  await page.fill("#username", username);
  await page.click("#register-button");
});
```

### 2. Use the `Server` enum for different server configs

```typescript
import { test, expect, Server } from "../utils/fixtures.ts";

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
} from "../utils/fixtures.ts";
import { getServer, Server } from "../utils/server.ts";
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

Defined in `Server` enum (`utils/server.ts`):
- `Server.Default` - Standard server
- `Server.NoSignup` - Server with `--no-signup` flag
- `Server.BasePath` - Server with `--base /crow-chipher`

## Adding a New Server Configuration

1. Add to `Server` enum in `utils/server.ts`
2. Add config to `SERVER_CONFIGS` object
3. It will be auto-started in global setup

## Test Spec Files

### Authentication & Users
- `register.spec.ts` - User registration flow
- `login.spec.ts` - Login flow and navigation
- `logout.spec.ts` - Logout behavior
- `app-auth.spec.ts` - Authentication, JWT, and authorization
- `claim.spec.ts` - Account claim flow
- `reclaim.spec.ts` - Account reclaim flow

### Admin
- `admin-claim.spec.ts` - Admin claim flow with encryption setup
- `admin-dashboard.spec.ts` - Admin dashboard access control and users table

### Posts
- `post-navigation.spec.ts` - Post switching, save behavior, editor state
- `new-post-sibling.spec.ts` - Creating sibling posts
- `last-selected-post.spec.ts` - Remembering last selected post
- `reorder.spec.ts` - Drag and drop post reordering

### Nested Posts (`specs/nested-posts/`)
- `nested-posts-basic.spec.ts` - Hierarchical post structure
- `nested-posts-delete.spec.ts` - Deleting nested posts
- `nested-posts-drag-drop.spec.ts` - Drag and drop for nested posts
- `nested-posts-expand-collapse.spec.ts` - Expand/collapse behavior
- `nested-posts-persistence.spec.ts` - Persistence of nested structure
- `nested-posts-reorder.spec.ts` - Reordering within nested structure
- `nested-posts-visual.spec.ts` - Visual rendering of nested posts
- `nested-posts-helpers.ts` - Shared helpers for nested post tests

### Uploads
- `upload.spec.ts` - Single image upload with progress and encryption
- `multi-upload.spec.ts` - Multi-image upload, gallery deletion, add to gallery

### Security & Tokens
- `encryption.spec.ts` - Encryption setup and usage
- `tokens.spec.ts` - Dual-token auth system (issuance, refresh, multi-session, revocation, login token rotation)

### UI Features
- `sidebar.spec.ts` - Sidebar behavior
- `settings-panel.spec.ts` - Settings panel UI
- `theme.spec.ts` - Theme switching
- `spellcheck.spec.ts` - Spellcheck toggle

### Infrastructure
- `base-path.spec.ts` - Base path (`--base`) support
