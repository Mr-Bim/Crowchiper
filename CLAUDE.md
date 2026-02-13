# Crowchiper

App with passkey authentication. Single binary with embedded frontend.

DO NOT USE tail or head for your commands.
DO always update the appropriate CLAUDE.md after a finished task.

## Commands

```bash
npm run build              # Build all frontends for production (login, app, dashboard)
npm run build:test         # Build all frontends with test mode (app has TEST_MODE=1)
npm run prepare-test       # Build frontend and rust in test mode, run before tests
npm run lint:fix           # TypeScript type check (tsc) and lint fix (oxlint)
npm run test:rust          # Run Rust tests (requires prior cargo build --features test-mode)
npm run test:web           # Run Playwright tests (requires prior build:test + cargo build)
npm run test:all           # Run both test:rust and test:web
cargo run -- --port 7291 --database crowchiper.db
cargo run -- --base /app   # With base path for reverse proxy
cargo run -- --no-signup   # Disable signups
cargo run -- --csp-nonce   # Add random nonce to CSP headers (for Cloudflare bot detection)
cargo run -- --plugin a.wasm --plugin b.wasm  # Load WASM plugins on startup
cargo run --features test-mode  # Run with test mode enabled
cargo build --release      # Release build (test-mode not included by default)
```

## URL Structure

- `/login/*` - Public login/register pages
- `/fiery-sparrow/*` - JWT-protected app
- `/dashboard/*` - JWT-protected admin dashboard
- `/api/*` - API endpoints (mixed auth)

With `--base /app`, all paths are prefixed.

## Conventional Commits

All commit messages must follow [Conventional Commits](https://www.conventionalcommits.org/).

**Format:** `<type>[optional scope]: <description>`

**Types:** `feat`, `fix`, `docs`, `style`, `refactor`, `perf`, `test`, `build`, `ci`, `chore`, `revert`

**Setup:** Run `git config core.hooksPath .githooks` to enable the commit-msg hook.

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
- **Use `.ts` extensions** for all relative imports in `web/` and `e2e/` folders

## CSS Minifier Control

### ID/Class Name Overlap Warning

The CSS minifier replaces class names in JS files too. If an HTML `id` has the same name as a CSS class, `getElementById` calls break. Build warns and skips minification when overlap is detected. **Fix:** Rename either the HTML ID or the CSS class.

### data-testid Handling

- **Production build**: `data-testid` attributes are stripped
- **Test build**: Preserved, but build fails if values overlap with CSS class names
- **Always prefix** `data-testid` values with `test-` to avoid conflicts with CSS class names

## Configuration

`config.json` defines shared settings for Vite and Rust:
- `assets`: App asset URL prefix (e.g., `/fiery-sparrow`)

## Global JS Variables

Set in `inline.ts`, available via `declare const`:
- `BASE_PATH`, `API_PATH`, `LOGIN_PATH`, `APP_PATH`

## Adding Pages

**Login page**: Create `web/public/my-page.html`, run `npm run build`
**App page**: Create `web/app/my-page.html`, run `npm run build`

## When changing the web folder/frontend ALWAYS
- Run npm run lint:fix
- Fix errors
- Remove unused code
- Create Playwright tests for new features or behavior changes (in `e2e/specs/`)
- Update the relevant CLAUDE.md file if there's something relevant for future development

## Key Architecture (details in memory files)

- **Auth**: Dual-token system (5min access + 2-week refresh). Auth module in `src/auth/`. Use `impl_has_auth_state!` macro for API state structs.
- **Frontend**: Code-split with lazy-loaded chunks. 50KB entry chunk limit. CSP with SRI hashes. New lazy chunks need shared dependency for modulepreload (see memory).
- **State**: Minimal reactive signals (`web/app/src/reactive.ts`). Autosave with 1.5s debounce. Eager encryption for pagehide safety.
- **Posts**: Hierarchical tree structure with unlimited nesting. Drag-and-drop for reorder/reparent.
- **Uploads**: Multi-stage progress (converting/compressing/encrypting/uploading). Gallery patterns in `patterns.ts`. HEIC conversion lazy-loaded.
- **DB layer**: `_tx` associated functions for cross-store transactions. `Database` has coordinating methods. API handlers must NOT contain raw SQL.
- **Build plugins**: `build-plugins/` folder. CSS minification, IIFE inlining, SRI, HTML processing.
- **Tests**: Rust tests in `tests/`. E2E in `e2e/` (see `e2e/CLAUDE.md`). PRF injection for WebAuthn testing. Plugin tests in `tests/plugin_tests.rs` (22 tests covering loading, sandbox, config validation, CLI, verbose/clean error modes).
- **Plugin system (WIP)**: Wasmtime + WIT component model. `--plugin path.wasm` CLI flag. Test plugins built with `wit-bindgen 0.53` targeting `wasm32-wasip2`. Build with `tests/plugins/build.sh`.
- **CI/CD**: `.github/workflows/` — CI on push/PR, release on version tag. Tag must match Cargo.toml version.

## Playwright E2E Tests

E2E tests located in `e2e/` folder. See `e2e/CLAUDE.md` for details.

**First-time setup**: Run `npx playwright install chromium` to download the browser.
