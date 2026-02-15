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
cargo run -- --plugin "a.wasm:net,env-HOME"    # Plugin with network + env permission (per-variable)
cargo run -- --plugin "a.wasm:fs-read=/data"  # Plugin with read-only filesystem access
cargo run -- --plugin "a.wasm:fs-write=/tmp"  # Plugin with read+write filesystem access
cargo run -- --plugin "a.wasm:var-key=value"  # Plugin with config variables
cargo run -- --plugin "a.wasm:timeout=10"     # Plugin with 10s hook timeout (default 5s)
cargo run -- --plugin "a.wasm:timeout=500ms"  # Plugin with 500ms hook timeout (min 10ms)
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

- **Auth**: Dual-token system (5min access + 2-week refresh). Auth module in `src/auth/`. Use `impl_has_db_jwt!` macro for API state structs (provides `jwt()` and `db()`).
- **Frontend**: Code-split with lazy-loaded chunks. 50KB entry chunk limit. CSP with SRI hashes. New lazy chunks need shared dependency for modulepreload (see memory).
- **State**: Minimal reactive signals (`web/app/src/reactive.ts`). Autosave with 1.5s debounce. Eager encryption for pagehide safety.
- **Posts**: Hierarchical tree structure with unlimited nesting. Drag-and-drop for reorder/reparent.
- **Uploads**: Multi-stage progress (converting/compressing/encrypting/uploading). Gallery patterns in `patterns.ts`. HEIC conversion lazy-loaded.
- **DB layer**: `_tx` associated functions for cross-store transactions. `Database` has coordinating methods. API handlers must NOT contain raw SQL.
- **Build plugins**: `build-plugins/` folder. CSS minification, IIFE inlining, SRI, HTML processing.
- **Tests**: Rust tests in `tests/`. E2E in `e2e/` (see `e2e/CLAUDE.md`). PRF injection for WebAuthn testing. Plugin tests in `tests/plugin_tests.rs` (48 tests) and `tests/plugin_permission_isolation_tests.rs` (7 tests verifying permissions don't leak between plugins).
- **Plugin system (WIP)**: Wasmtime + WIT component model. `--plugin path.wasm[:perm1,perm2,var-key=val,timeout=5|500ms]` CLI flag. Per-plugin permissions: `net`, `env-<VAR_NAME>`, `fs-read=<path>`, `fs-write=<path>`. `net` grants full TCP/UDP (wasmtime WASI limitation). Config variables via `var-<key>=<value>` passed as `list<tuple<string, string>>` to the plugin's `config()` function. Resource limits: 10M fuel (CPU), 10MB memory, 512KB stack, wall-clock timeout (default 5s, min 10ms, configurable via `timeout=<secs>` or `timeout=<N>ms`). The timeout covers both WASM execution and time in async host calls (network/fs I/O). Two-layer defense: `fuel_async_yield_interval(10_000)` ensures WASM yields to tokio for timeout checking, `tokio::time::timeout` enforces the wall-clock bound. A plugin that times out has its instance dropped (the store is in an undefined state) and is **automatically reloaded from disk** on the next hook call. Filesystem paths must be absolute and are canonicalized at load time. Permissions module in `src/plugin/permissions.rs`. Test plugins built with `wit-bindgen 0.53` targeting `wasm32-wasip2`. Build with `tests/plugins/build.sh`.
- **Plugin hooks**: Plugins export `on-hook(event) -> result<_, string>` to react to server events. Hooks are WIT enums grouped by target: `variant hook { server(server-hook) }` with `server-hook { ip-change }`. Each plugin declares a `target` (server only for now) in `plugin-config` and registers specific hooks — validated at load time to match target. `PluginManager` dispatches hooks asynchronously: different plugins run concurrently via `futures::future::join_all`, same plugin serializes via `tokio::sync::Mutex` (WASM is single-threaded). Plugin runtime uses `async_support(true)` + `add_to_linker_async` so WASI I/O yields to tokio. Called via `tokio::spawn` from async code. First hook: `ip-change` fired in `src/auth/extractors.rs` when user IP changes during token refresh (values: `old_ip`, `new_ip`, `user_uuid`).
- **Plugin module structure**: `src/plugin/` split into: `mod.rs` (WIT bindgen), `runtime.rs` (PluginRuntime load/hook), `manager.rs` (PluginManager dispatch), `state.rs` (WASI state + host log import), `helpers.rs` (permissions, sanitization, panic extraction), `permissions.rs` (CLI parsing), `error.rs` (PluginError).
- **Plugin logging**: Plugins call the host-provided `log(level, msg)` import to write to the server log. Levels: `debug`, `info`, `warn`, `error` — mapped to `tracing` macros. Output is sanitized against log injection (ANSI escapes, control characters, newlines stripped). Messages capped at 4KB per call. Implemented via `PluginImports` trait in `src/plugin/state.rs`. WIT definition in `wit/plugin.wit`.
- **CI/CD**: `.github/workflows/` — CI on push/PR, release on version tag. Tag must match Cargo.toml version.

## Playwright E2E Tests

E2E tests located in `e2e/` folder. See `e2e/CLAUDE.md` for details.

**First-time setup**: Run `npx playwright install chromium` to download the browser.
