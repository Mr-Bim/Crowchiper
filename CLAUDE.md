# Crowchiper

App with passkey authentication. Single binary with embedded frontend.

DO NOT USE tail or head for your commands.
DO always update the appropriate CLAUDE.md after a finished task.

## Commands

```bash
npm run build-all          # Build frontend with test mode (default for development)
npm run build-all-release  # Build frontend for production (no test mode)
npm run lint:fix           # TypeScript type check and fix
cargo run -- --port 7291 --database crowchiper.db
cargo run -- --base /app   # With base path for reverse proxy
cargo run -- --no-signup   # Disable signups
cargo test --tests -- --test-threads=1  # Run all tests (test-mode enabled by default)
cargo build --release --no-default-features  # Release build without test-mode
```

## URL Structure

- `/` redirects to `/login/`
- `/login/*` - Public login, register, claim pages
- `/fiery-sparrow/*` - JWT-protected app
- `/api/*` - API endpoints (mixed auth)

With `--base /app`, all paths are prefixed.

## Development Rules

- Run `cargo build` when finished with new functionality
- Write tests for new functionality and run all tests before finishing
- Run `npm run link:fix` when changing frontend code
- Never expose internal database IDs - use UUIDs for API responses
- Use data attributes for JS-controlled visibility (CSS minifier mangles class names)
- Page-specific CSS in separate files, not inline `<style>` blocks

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
- Run npm run check
- Fix errors
- Remove unused code
- Update the relevant CLAUDE.md file if there's something relevate for future development

## Testing Encryption Without PRF

Chrome's virtual authenticator doesn't support PRF. Test mode is enabled by default for development:

1. Build frontend: `npm run build-all` (test mode included by default)
2. Run tests: `cargo test --tests -- --test-threads=1` (test-mode feature enabled by default)
3. In tests, use `TestContext::enable_test_encryption(user_id)` to:
   - Enable encryption for the user in the database (no PRF salt)
   - Generate and inject a test key via `window.__TEST_ENCRYPTION_KEY__`
4. The frontend checks for this global and imports it directly
5. Use `AddScriptToEvaluateOnNewDocumentParams` to inject the key before page scripts run

Example test usage:
```rust
use chromiumoxide::cdp::browser_protocol::page::AddScriptToEvaluateOnNewDocumentParams;

let test_key = ctx.enable_test_encryption(user_id).await;

// Inject key before navigation so it's available when main.ts initializes
let script = format!("window.__TEST_ENCRYPTION_KEY__ = '{}';", test_key);
ctx.page
    .execute(AddScriptToEvaluateOnNewDocumentParams::new(script))
    .await
    .expect("Failed to inject test key");

ctx.page.goto(&app_url).await.expect("Failed to navigate");
```

The test code is stripped from release/production builds:
- Rust: Use `cargo build --release --no-default-features` to exclude test-mode
- JS: Use `npm run build-all-release` to build with RELEASE_MODE (strips test code)
- `__RELEASE_MODE__` constant is replaced at build time and dead code is eliminated

## Save Button

The app header includes a Save button that:
- Shows "Saved" (disabled) when there are no unsaved changes
- Shows "Save" (clickable, highlighted) when there are unsaved changes
- Uses `data-dirty` attribute for styling (`data-dirty="true"` or `data-dirty="false"`)
- Located in `web/app/index.html`, styled in `web/app/app.css`
- Functionality in `web/app/src/posts/ui.ts` (`handleSave`, `updateSaveButton`)
