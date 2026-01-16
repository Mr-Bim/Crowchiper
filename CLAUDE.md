# Crowchiper

App with passkey authentication. Single binary with embedded frontend.

DO NOT USE tail or head for your commands.
DO always update the appropriate CLAUDE.md after a finished task.

## Commands

```bash
npm run build-all          # Build frontend for production (no test mode)
npm run build-all-test     # Build frontend with test mode
npm run lint:fix           # TypeScript type check (tsc) and lint fix (oxlint)
npm run test:rust          # Build Rust with test-mode and run Rust tests
npm run test:web           # Build frontend+Rust with test-mode, run Playwright tests
npm run test:all           # Run both test:rust and test:web
cargo run -- --port 7291 --database crowchiper.db
cargo run -- --base /app   # With base path for reverse proxy
cargo run -- --no-signup   # Disable signups
cargo run --features test-mode  # Run with test mode enabled
cargo build --release      # Release build (test-mode not included by default)
```

## URL Structure

- `/` redirects to `/login/`
- `/login/*` - Public login, register, claim pages
- `/fiery-sparrow/*` - JWT-protected app
- `/api/*` - API endpoints (mixed auth)

With `--base /app`, all paths are prefixed.

## Development Rules

- Run `cargo build` when finished with new functionality
- Write tests for new functionality and run tests before finishing:
  - `npm run test:rust` for backend-only changes
  - `npm run test:web` for frontend-only changes
  - `npm run test:all` for features touching both frontend and backend
- Run `npm run lint:fix` when changing frontend code
- Never expose internal database IDs - use UUIDs for API responses
- Use data attributes for JS-controlled visibility (CSS minifier mangles class names)
- Page-specific CSS in separate files, not inline `<style>` blocks
- **Use `.ts` extensions** for all relative imports in `web/` and `e2e/` folders (e.g., `import { foo } from "./bar.ts"`)

## CSS Minifier Control

```css
.gl-minify-disable-NAME { --marker: 1 }  /* turns OFF minification */
.gl-minify-enable-NAME { --marker: 1 }   /* turns it back ON */
```

## Configuration

`config.json` defines shared settings for Vite and Rust:
- `assets`: App asset URL prefix (e.g., `/fiery-sparrow`)

## Global JS Variables

Set in `inline.ts`, available via `declare const`:
- `BASE_PATH` - Base path without trailing slash
- `API_PATH` - API base path
- `LOGIN_PATH` - Login pages path
- `APP_PATH` - App pages path

## Adding Pages

**Login page**: Create `web/public/my-page.html`, run `npm run build-all`
**App page**: Create `web/app/my-page.html`, run `npm run build-all`

## When changing the web folder/frontend ALWAYS
- Run npm run lint:fix
- Fix errors
- Remove unused code
- Update the relevant CLAUDE.md file if there's something relevant for future development

## Testing Encryption (PRF Injection)

Chrome's virtual authenticator has two limitations:
1. Doesn't return PRF output (even with `hasPrf: true`)
2. Doesn't support discoverable credentials

Test mode injects values to work around these limitations:

**Window globals (injected via `addInitScript`):**
- `__TEST_PRF_OUTPUT__` - Base64url-encoded 32-byte PRF output
- `__TEST_USERNAME__` - Username for passkey authentication (bypasses discoverable credential requirement)

**How it works:**
1. `extractPrfOutput()` in `crypto/operations.ts` checks for `__TEST_PRF_OUTPUT__` first
2. `createUnlockHandler()` in `unlock/index.ts` checks for `__TEST_USERNAME__` to use non-discoverable auth
3. `handleTestPrf()` in `setup-encryption.ts` checks for `__TEST_USERNAME__` for the PRF test

**Playwright example:**
```typescript
import {
  addVirtualAuthenticator,
  generateTestPrfOutput,
  injectTestPrfOutput,
  injectTestUsername,
} from "./fixtures.ts";

const page = await context.newPage();
const client = await page.context().newCDPSession(page);
await addVirtualAuthenticator(client);

const username = "testuser";
await injectTestPrfOutput(page, generateTestPrfOutput());
await injectTestUsername(page, username);

await page.goto(`${baseUrl}/login/register.html`);
// ... register and test encryption flow
```

The test code is only included when building with test mode:
- JS: Use `npm run build-all-test` to build with TEST_MODE (includes test code)
- `__TEST_MODE__` constant is replaced at build time and dead code is eliminated

## Gallery/Attachment Patterns

Shared regex patterns for gallery parsing are in `web/app/src/editor/attachment-widget/patterns.ts`:
- `GALLERY_PATTERN` - Match gallery syntax anywhere in text
- `GALLERY_LINE_PATTERN` - Match gallery syntax on a single line (with ^ anchor)
- `GALLERY_IMAGE_PATTERN` - Extract individual images from gallery content

Always import and reuse these patterns instead of defining new regex for galleries.

## Save Button

The app header includes a Save button that:
- Shows "Saved" (disabled) when there are no unsaved changes
- Shows "Save" (clickable, highlighted) when there are unsaved changes
- Uses `data-dirty` attribute for styling (`data-dirty="true"` or `data-dirty="false"`)
- Located in `web/app/index.html`, styled in `web/app/app.css`
- Functionality in `web/app/src/posts/ui.ts` (`handleSave`, `updateSaveButton`)

## Playwright E2E Tests

E2E tests located in `e2e/` folder. See `e2e/CLAUDE.md` for details.

**First-time setup**: Run `npx playwright install chromium` to download the browser.
