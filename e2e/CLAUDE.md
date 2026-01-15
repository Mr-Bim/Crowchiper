# E2E Tests

Playwright tests using Lightpanda browser.

## Commands

```bash
npx playwright test        # Run all e2e tests
npx playwright test --ui   # Run with UI mode
```

## Important: Use .ts Extensions

All relative imports MUST include the `.ts` extension:

```typescript
// Correct
import { test, expect } from "./fixtures.ts";
import { getServer } from "./server.ts";

// Wrong - will fail
import { test, expect } from "./fixtures";
import { getServer } from "./server";
```

This applies to the web folder as well.

## Files

- `fixtures.ts` - Test fixtures (`page`, `baseUrl`, `serverWithOptions`)
- `server.ts` - Server manager for lazy-loading Crowchiper servers
- `global-setup.ts` - Starts Lightpanda and default server
- `global-teardown.ts` - Stops all servers and Lightpanda

## Writing Tests

```typescript
import { test, expect } from "./fixtures.ts";

// Use default server
test("basic test", async ({ page, baseUrl }) => {
  await page.goto(`${baseUrl}/login/`);
});

// Use server with specific options (lazy-loaded, cached)
test("no-signup test", async ({ page, serverWithOptions }) => {
  const { baseUrl } = await serverWithOptions({ noSignup: true });
  await page.goto(`${baseUrl}/login/`);
});
```

## Server Options

Available options for `serverWithOptions()`:

- `noSignup: boolean` - Start server with `--no-signup` flag
- `base: string` - Start server with `--base` path (e.g., `/app`)

Servers are cached by config - requesting the same options twice returns the same server.
