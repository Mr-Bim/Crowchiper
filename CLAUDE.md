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

All commit messages must follow the [Conventional Commits](https://www.conventionalcommits.org/) specification.

**Format:** `<type>[optional scope]: <description>`

**Valid types:**
- `feat` - A new feature
- `fix` - A bug fix
- `docs` - Documentation only changes
- `style` - Changes that do not affect the meaning of the code
- `refactor` - A code change that neither fixes a bug nor adds a feature
- `perf` - A code change that improves performance
- `test` - Adding missing tests or correcting existing tests
- `build` - Changes that affect the build system or dependencies
- `ci` - Changes to CI configuration files and scripts
- `chore` - Other changes that don't modify src or test files
- `revert` - Reverts a previous commit

**Examples:**
```
feat: add user authentication
fix(auth): resolve login timeout issue
docs: update API documentation
feat!: breaking change to API
```

**Setup:** Run `git config core.hooksPath .githooks` to enable the commit-msg hook that validates messages locally.

**Release notes:** Generated automatically from conventional commits using [git-cliff](https://git-cliff.org/) (configured in `cliff.toml`).

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


### ID/Class Name Overlap Warning

The CSS minifier also replaces class names in JS files (for `getElementById`, `classList.add`, etc.). If an HTML `id` attribute has the same name as a CSS class, this would break `getElementById` calls.

**Example problem:**
```html
<div id="unlock-overlay"></div>  <!-- HTML id -->
```
```css
.unlock-overlay { ... }  /* CSS class with same name */
```
```js
document.getElementById("unlock-overlay")  // Would get minified to "aa" and break!
```

When overlap is detected, the build will:
1. Print a warning with the overlapping names
2. Skip CSS minification for that file

**To fix:** Rename either the HTML ID or the CSS class so they don't share names.

### data-testid Handling

The build system handles `data-testid` attributes differently based on test mode:

**Production build (`npm run build`):**
- All `data-testid` attributes are stripped from HTML

**Test build (`npm run build:test`):**
- `data-testid` attributes are preserved
- Build fails if any `data-testid` value in HTML overlaps with a CSS class name

**IMPORTANT: Naming Convention**

Always prefix `data-testid` values with `test-` to avoid conflicts with CSS class names:
```typescript
// GOOD - uses test- prefix, won't conflict with .post-wrapper class
element.setAttribute("data-testid", "test-post-wrapper");

// BAD - same name as CSS class .post-wrapper, will get mangled by minifier
element.setAttribute("data-testid", "post-wrapper");
```

The CSS minifier replaces class name strings in JS files. If a `data-testid` value matches a CSS class name, it will be incorrectly replaced with the minified class name.

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

**Login page**: Create `web/public/my-page.html`, run `npm run build`
**App page**: Create `web/app/my-page.html`, run `npm run build`

## When changing the web folder/frontend ALWAYS
- Run npm run lint:fix
- Fix errors
- Remove unused code
- Create Playwright tests for new features or behavior changes (in `e2e/specs/`)
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
- JS: Use `npm run build:test` to build with TEST_MODE (includes test code)
- `__TEST_MODE__` constant is replaced at build time and dead code is eliminated

## Test Token Generation API

For e2e tests that need to create JWT tokens (e.g., testing token refresh flows), use the test API endpoint instead of client-side JWT libraries:

**Endpoint:** `POST /api/test/generate-tokens` (test-mode only)

**Request:**
```json
{
  "user_uuid": "uuid-string",
  "username": "testuser",
  "role": "user" | "admin",      // optional, defaults to "user"
  "ip_addr": "127.0.0.1",        // optional
  "expired_access": false,       // optional, generates expired access token
  "store_refresh": false         // optional, stores refresh token in DB
}
```

**Response:**
```json
{
  "access_token": "jwt...",
  "refresh_token": "jwt...",
  "refresh_jti": "uuid",
  "issued_at": 1234567890,
  "expires_at": 1234567890
}
```

**Usage in tests:**
```typescript
async function generateTokens(baseUrl: string, userUuid: string, username: string, options = {}) {
  const response = await fetch(`${baseUrl}/api/test/generate-tokens`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ user_uuid: userUuid, username, ...options }),
  });
  return response.json();
}

// Generate tokens with expired access (for testing refresh flow)
const tokens = await generateTokens(baseUrl, userUuid, username, {
  expired_access: true,
  store_refresh: true,
});
```

## Gallery/Attachment Patterns

Shared regex patterns for gallery parsing are in `web/app/src/editor/attachment-widget/patterns.ts`:
- `GALLERY_PATTERN` - Match gallery syntax anywhere in text
- `GALLERY_LINE_PATTERN` - Match gallery syntax on a single line (with ^ anchor)
- `GALLERY_IMAGE_PATTERN` - Extract individual images from gallery content
- `sanitizeAltText(alt)` - Sanitize alt text to prevent XSS and formatting issues

Always import and reuse these patterns instead of defining new regex for galleries.

## Upload Progress Feedback

Image uploads display granular progress through multiple stages:

### Progress Stages (`web/app/src/editor/attachment-widget/progress.ts`)
1. **Converting** - HEIC to WebP conversion (HEIC files only)
2. **Compressing** - Image compression and thumbnail generation
3. **Encrypting** - Encrypting image and thumbnails (if encryption enabled)
4. **Uploading** - Network upload with percentage (0-100%)

### HEIC Conversion
HEIC files (Apple's image format) require conversion to WebP before upload:
- **Warning modal** - Shows before conversion with estimated time (10-30 seconds per image)
- **Abort button** - X button on image placeholder to cancel conversion/upload
- **Lazy-loaded** - The `heic-to` library (2.5MB) is only loaded when HEIC files are detected
- **Key file**: `heic-convert.ts` - `convertHeicIfNeeded()`, `showHeicConversionModal()`

### Placeholder Format
During upload, a temporary placeholder is inserted in the editor:
```
![stage](attachment:upload-N)
![uploading:45](attachment:upload-N)
```
- `upload-N` or `widget-upload-N` - Unique ID for tracking
- Alt text contains stage name or `uploading:percent`

### Upload Abort/Cleanup
Uploads can be aborted when switching posts or cleaning up:
- `abortAllUploads()` - Aborts all active uploads (called in `selectPost()`)
- `registerUpload(id)` / `unregisterUpload(id)` - Track active uploads
- `cleanupPendingUploads(content)` - Removes placeholder text from content

All abort functions are in `web/app/src/shared/attachment-utils.ts` to avoid pulling editor chunk into main bundle.

### Key Files
- **`shared/attachment-utils.ts`** - Upload tracking, abort, cleanup functions
- **`progress.ts`** - Progress types and `getProgressText()` helper
- **`upload.ts`** - `processAndUploadFile()` with `onProgress` and `signal` for abort
- **`widget.ts`** - `renderImage()` parses placeholder, displays progress, adds abort button
- **`api/attachments.ts`** - `uploadAttachmentWithProgress()` uses XMLHttpRequest with AbortSignal
- **`heic-convert.ts`** - HEIC detection, conversion, warning modal

### Adding Progress to Custom Uploads
```typescript
import { processAndUploadFile, type UploadProgress } from "./upload.ts";
import { registerUpload, unregisterUpload } from "../../shared/attachment-utils.ts";

const uploadId = "my-upload-1";
const controller = registerUpload(uploadId);

const uuid = await processAndUploadFile(file, {
  onProgress: (progress: UploadProgress) => {
    console.log(progress.stage, progress.percent);
  },
  isCancelled: () => false,
  signal: controller.signal,
});

unregisterUpload(uploadId);
```

## Security Utilities

### Request Timeouts
`fetchWithAuth()` in `web/app/src/api/auth.ts` includes a 30-second timeout by default to prevent indefinite hangs. Custom timeout can be passed via `timeoutMs` option.

### UUID Validation
Drag-and-drop operations validate UUIDs from data attributes before processing to prevent injection attacks. See `isValidUuid()` in `web/app/src/posts/drag-and-drop.ts`.

### Sensitive Buffer Clearing
`secureClear(buffer)` in `web/app/src/crypto/operations.ts` overwrites ArrayBuffers with zeros after use. Used automatically in:
- `deriveEncryptionKeyFromPrf()` - Clears PRF output after key derivation
- `decryptBinary()` - Clears ciphertext after decryption

Note: Due to JavaScript's memory model, this is best-effort only.

## Code Splitting Structure

The app uses code splitting to keep the initial bundle small (~17KB). Heavy features are lazy-loaded.

### Entry Point (`web/app/src/main.ts`)
Loaded immediately on page load. Contains:
- Authentication verification
- Encryption key management
- Post list state and rendering
- Unlock overlay UI

### Lazy-Loaded Chunks
1. **Drag-and-drop chunk** (`posts/drag-and-drop.ts`) - Post reordering (~27KB), loaded when posts are rendered
2. **Editor chunk** (`editor/setup.ts`) - CodeMirror + plugins (~316KB), loaded when `setupEditor()` is first called
3. **Attachment widget chunk** (`editor/attachment-widget/index.ts`) - Gallery handling (~238KB), loaded inside `loadPosts()` after unlock
4. **HEIC converter** (`heic-to` library) - Loaded only when uploading HEIC images (~2.5MB)

**Important:** These chunks must NOT be imported at module top-level, or they will block the unlock overlay from appearing quickly. Use lazy `import()` inside functions instead.

### CSP and Dynamic Imports

The app uses CSP with `'strict-dynamic'` to allow dynamically loaded scripts. However, **ES module dynamic imports (`import()`) are NOT covered by `strict-dynamic`** according to the W3C CSP spec. Browsers block dynamically imported modules unless they are modulepreloaded with integrity hashes.

**How Vite handles this:**
- Vite's preload polyfill (`Y` function in bundled code) creates `<link rel="modulepreload">` tags dynamically
- These tags include `integrity` attributes that match hashes in the CSP
- Chunks are only added to the preload list (`__vite__mapDeps`) if they share dependencies with other chunks
- When a chunk has shared dependencies, Vite creates the modulepreload link **just before** the import, not at page load

**The Problem:**
When a chunk has no shared dependencies with other chunks, Vite imports it with an empty deps array, skipping the modulepreload step. The browser then blocks it due to CSP.

**The Fix:**
Add an import from a shared module (like `shared/dom.ts`) to create a dependency link:

```typescript
// In the lazy-loaded chunk (e.g., drag-and-drop.ts):
// Import from shared/dom to create a shared dependency with other chunks.
// This allows Vite to modulepreload this chunk dynamically (not eagerly),
// which is required for CSP strict-dynamic to work with ES module imports.
import { getOptionalElement } from "../../../shared/dom.ts";

// Use the import somewhere to prevent tree-shaking
if (!getOptionalElement(container.id)) return;
```

This makes Vite:
1. Include the chunk in `__vite__mapDeps` array
2. Create modulepreload links dynamically when the chunk is needed (not at page load)
3. Include integrity hashes that satisfy CSP

**When adding new lazy-loaded chunks:**
1. Build and check the bundled index.js for the chunk's import
2. If it uses `__vite__mapDeps([])` (empty array), the chunk has no shared dependencies
3. Add an import from `shared/dom.ts` and use it somewhere in the code
4. Rebuild both frontend (`npm run build`) and Rust (`cargo build`) to update CSP hashes
5. Verify the chunk now has a non-empty `__vite__mapDeps` array

### Shared Utilities (`web/app/src/shared/`)
Utilities used by both the main bundle and lazy chunks. Import from here to avoid pulling editor dependencies into the main bundle:

- **`attachment-utils.ts`** - `parseAttachmentUuids()`, `cleanupPendingUploads()`
- **`image-cache.ts`** - `thumbnailCache`, `fullImageCache`, `clearImageCache()`, `clearImageCacheExcept()`
- **`index.ts`** - Barrel export for all shared utilities

### DOM Utilities (`web/shared/dom.ts`)
Type-safe DOM query helpers shared across login and app builds:

- **`getRequiredElement(id, type?)`** - Get element by ID, throws if not found
- **`getOptionalElement(id, type?)`** - Get element by ID, returns null if not found

```typescript
// Throws if element doesn't exist or isn't an HTMLButtonElement
const btn = getRequiredElement("submit-btn", HTMLButtonElement);

// Returns null if element doesn't exist
const optional = getOptionalElement("maybe-exists");
```

**Example:**
```typescript
// GOOD - imports from shared, stays in main bundle
import { parseAttachmentUuids } from "../shared/attachment-utils.ts";

// AVOID - would pull in editor chunk dependencies
import { parseAttachmentUuids } from "../editor/attachment-widget/utils.ts";
```

The `editor/attachment-widget/utils.ts` and `cache.ts` files re-export from shared for backward compatibility within the editor chunk.

## Autosave and Sync Indicator

The app automatically saves content after 5 seconds of inactivity (autosave with debounce). The header shows a sync indicator instead of a save button:

### Sync States (`SyncStatus` in `web/app/src/posts/state/signals.ts`)
- **idle** - No pending changes, indicator hidden
- **pending** - Changes waiting to be saved (faded checkmark)
- **syncing** - Save in progress (spinning indicator)
- **synced** - Save completed successfully (green checkmark with pop animation, returns to idle after 2s)
- **error** - Save failed (red X icon)

### Key Files
- **State**: `web/app/src/posts/state/signals.ts` - `syncStatusSignal`, `SyncStatus` type
- **Save logic**: `web/app/src/posts/save.ts` - `scheduleAutosave()`, `forceSave()`, `saveToServerNow()`
- **UI updates**: `web/app/src/posts/subscriptions.ts` - Subscribes to `syncStatusSignal`
- **HTML**: `web/app/index.html` - `#sync-indicator` with `data-status` attribute
- **CSS**: `web/app/css/app.css` - `.cl-sync-indicator` styles and animations

### Test Mode Force Save Button
In test mode (`__TEST_MODE__`), a "Save" button is dynamically added next to the sync indicator for testing purposes. This allows tests to trigger immediate saves instead of waiting for the 5-second debounce.

```typescript
// In tests, use the force save button instead of waitForTimeout
const forceSaveBtn = page.locator('[data-testid="test-force-save-btn"]');
const syncIndicator = page.locator('[data-testid="test-sync-indicator"]');

await forceSaveBtn.click();
await expect(syncIndicator).toHaveAttribute("data-status", "synced", { timeout: 5000 });
```

## Reactive State Management

The app uses a minimal reactive signal primitive for automatic UI updates.

### Signal Primitive (`web/app/src/reactive.ts`)
A ~50 line reactive primitive providing:
- `signal<T>(initial)` - Create a reactive value
- `computed(deps, fn)` - Create a derived value from other signals

```typescript
import { signal } from "../reactive.ts";

const count = signal(0);
count.get();              // Read: 0
count.set(5);             // Write (notifies subscribers)
count.update(n => n + 1); // Update with function

// Subscribe to changes
const unsubscribe = count.subscribe((value) => {
  console.log("Count:", value);
});
```

### Exported Signals (`web/app/src/posts/state.ts`)
- `isDirtySignal` - Whether there are unsaved changes
- `postsSignal` - Tree structure of posts
- `loadedPostSignal` - Currently selected post
- `editorSignal` - Active CodeMirror instance

Backward-compatible getter/setter functions still work (e.g., `getIsDirty()`, `setIsDirty()`).

### Subscriptions (`web/app/src/posts/subscriptions.ts`)
Reactive subscriptions are initialized in `main.ts` via `initSubscriptions()`:
- Save button automatically updates when `isDirtySignal` changes

### Adding New Reactive UI
```typescript
// In subscriptions.ts
import { someSignal } from "./state.ts";

export function initSubscriptions(): void {
  // ... existing subscriptions
  
  someSignal.subscribe((value) => {
    // Update DOM based on value
  });
}
```

### Type Definitions (`web/app/src/posts/types.ts`)
Formalizes contracts between modules:
- `PostsState` - Complete state shape
- `PostHandlers` - Handler registry interface
- `PendingEncryptedData` - Encrypted data awaiting server save
- `DragData`, `DropLocation`, `DropAction` - Drag-and-drop types

## Nested Posts (Hierarchical Structure)

Posts support unlimited nesting depth, similar to Notion. Any post can have child posts.

### Database Schema
- `parent_id`: References parent post's UUID (NULL = root level)
- Positions are scoped per-parent (siblings ordered 0, 1, 2...)
- `ON DELETE CASCADE`: Deleting a parent deletes all children

### API Endpoints
- `GET /posts` - Returns tree structure (1 level deep by default)
- `GET /posts/{uuid}/children` - Lazy load children beyond initial depth
- `POST /posts` - Accepts `parent_id` field
- `POST /posts/{uuid}/move` - Move post to new parent: `{ parent_id, position }`
- `POST /posts/reorder` - Reorder siblings: `{ parent_id, uuids }`
- `DELETE /posts/{uuid}` - Returns `{ deleted, children_deleted }` count

### Frontend Types (`web/app/src/api/posts.ts`)
```typescript
interface PostNode {
  uuid: string;
  title: string | null;
  parent_id: string | null;
  has_children: boolean;
  children: PostNode[] | null;  // null = not yet loaded
  // ... other fields
}
```

### State Management (`web/app/src/posts/state.ts`)
- `posts` is now a tree structure (`PostNode[]`)
- `expandedPosts: Set<string>` tracks expanded posts
- Helper functions: `findPost()`, `findParent()`, `movePostInTree()`
- Uses reactive signals for state that triggers UI updates (see Reactive State section)

### UI Behavior (`web/app/src/posts/render.ts`)
- Tree rendered with indentation (16px per level)
- Expand/collapse chevrons for posts with children
- Click post to select for editing
- First 3 levels expanded by default
- Delete button appears on hover

### Drag-and-Drop (`web/app/src/posts/drag-and-drop.ts`)
Two drop modes based on pointer position:
- **Edges**: Reorder as sibling (drop line above/below)
- **Center**: Reparent (highlight on post)

### Delete Behavior
- Deleting a post with children shows warning with count
- User must confirm before cascade delete

## Dual-Token Authentication

The app uses a dual-token system with access tokens and refresh tokens:

### Token Types
- **Access tokens**: Short-lived (5 minutes), stateless, no JTI. Used for API authentication.
- **Refresh tokens**: Long-lived (2 weeks), tracked in database with JTI. Used to obtain new access tokens.

### How It Works
1. On login, a refresh token is issued only if the user doesn't already have a valid one
2. API requests use the access token for authentication
3. If access token is expired but refresh token is valid, middleware auto-refreshes the access token
4. If both tokens are invalid/expired, returns 401 and frontend redirects to login
5. Refresh tokens can be revoked (logged out) which invalidates the session
6. IP address is validated on access tokens; if IP changes, refresh token is used to issue new access token

### Database Schema (v9)
```sql
CREATE TABLE active_tokens (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    jti TEXT UNIQUE NOT NULL,      -- JWT ID (refresh tokens only)
    user_id INTEGER NOT NULL,      -- References users(id)
    last_ip TEXT,                  -- Last IP address used
    issued_at TEXT NOT NULL,       -- When token was issued
    expires_at TEXT NOT NULL,      -- When token expires
    token_type TEXT NOT NULL,      -- Always 'refresh'
    created_at TEXT NOT NULL
)
```

### JWT Claims
Access tokens (`src/jwt.rs`):
```rust
struct AccessClaims {
    sub: String,      // User UUID
    username: String,
    role: UserRole,
    typ: TokenType,   // "access"
    iat: u64,
    exp: u64,
}
```

Refresh tokens include additional `jti` field for database tracking.

### Cookie Names
- `access_token` - Short-lived access token
- `refresh_token` - Long-lived refresh token (tracked in DB)

### API Endpoints
- `POST /api/tokens/refresh` - Exchange refresh token for new access token
- `POST /api/tokens/logout` - Revoke refresh token and clear both cookies
- `GET /api/tokens` - List user's active refresh tokens
- `GET /api/tokens/verify` - Check if current access token is valid
- `DELETE /api/tokens/{jti}` - Revoke specific refresh token

### Frontend Behavior
- On 401 response, frontend redirects to login page (`fetchWithAuth` in `web/app/src/api/auth.ts`)
- No client-side token refresh logic needed - server handles it automatically

### Frontend Settings Menu
The sidebar footer has a settings menu (gear icon) with:
- Theme selector dropdown (logic in `web/inline/inline.ts`, shared between login and app)
- Logout button (calls `/api/tokens/logout`, redirects to login)
- Version button (opens version modal)

Located in `web/app/index.html`, styled in `web/app/css/app.css`, app-specific logic (settings menu toggle, logout, version modal) in `web/app/src/main.ts`.

### Token Cleanup
Expired tokens are deleted on server startup via `db.tokens().delete_expired()`.

### Auth Module Structure (`src/auth/`)

The authentication logic is organized into focused submodules:

- **`mod.rs`** - Main module with re-exports for backward compatibility
- **`cookie.rs`** - Cookie parsing (`get_cookie`, cookie name constants)
- **`errors.rs`** - Error types (`ApiAuthError`, `AssetAuthError`, `AuthErrorKind`)
- **`extractors.rs`** - Axum extractors (`ApiAuth`, `ActivatedApiAuth`, `AssetAuth`, `MaybeAuth`)
- **`ip.rs`** - Client IP extraction (`extract_client_ip`, `HasHeadersAndExtensions` trait)
- **`state.rs`** - State traits (`HasAuthState`, `HasAssetAuthState`) and `impl_has_auth_state!` macro
- **`types.rs`** - User types (`AuthenticatedUser`, `ActivatedAuthenticatedUser`)

### HasAuthState Macro

API state structs use the `impl_has_auth_state!` macro to avoid boilerplate:

```rust
use crate::impl_has_auth_state;

#[derive(Clone)]
pub struct MyApiState {
    pub db: Database,
    pub jwt: Arc<JwtConfig>,
    pub secure_cookies: bool,
    pub ip_extractor: Option<IpExtractor>,
    // ... other fields
}

impl_has_auth_state!(MyApiState);
```

The macro implements `HasAuthState` trait methods (`jwt()`, `db()`, `secure_cookies()`, `ip_extractor()`).

## Admin Dashboard

The admin dashboard (`/dashboard/`) lists all users and is only accessible to admin users.

### User Settings Endpoint

`GET /api/user/settings` replaces the old `GET /api/encryption/settings`. Returns encryption settings plus admin info:
```json
{
  "setup_done": false,
  "encryption_enabled": false,
  "prf_salt": null,
  "is_admin": true,
  "dashboard_path": "/dashboard"
}
```
- `is_admin` and `dashboard_path` are used by the app frontend to show a "Dashboard" link in the settings menu
- `dashboard_path` is only included for admins (omitted via `skip_serializing_if` for non-admins)

### Admin API

- `GET /api/admin/users` - Lists all activated users (admin-only, returns 403 for non-admins)
- Response: `[{ uuid, username, role, activated, created_at }]` — no internal database IDs

### Admin Claim Flow

Admins are created via `--create-admin` CLI flag, which outputs a claim URL (`/login/claim.html?uuid=...`). The claim page registers a passkey, then redirects to `/app/setup-encryption.html` for encryption setup — the same flow as regular user registration. After encryption setup, the admin can access both the app and the dashboard.

### Key Files
- **`src/api/user_settings.rs`** - User settings endpoint (encryption + admin info)
- **`src/api/admin.rs`** - Admin API (user listing)
- **`src/db/user.rs`** - `UserSummary` struct, `list_activated()` method
- **`web/login/js/claim.ts`** - Admin claim page (passkey registration + redirect to encryption setup)
- **`web/dashboard/`** - Dashboard frontend (HTML, CSS, JS)
- **`web/app/index.html`** - Dashboard link in settings menu (hidden for non-admins)

## Database Layer Patterns

### Transaction-Aware Methods

Stores (`PostStore`, `AttachmentStore`) provide `_tx` associated functions that accept `&mut sqlx::Transaction` for use in cross-store transactions:

```rust
// Associated functions (no &self) - operate on a transaction
PostStore::get_id_by_uuid_tx(&mut tx, uuid, user_id)
PostStore::update_tx(&mut tx, uuid, user_id, ...)
PostStore::get_descendant_ids_tx(&mut tx, uuid, user_id)
PostStore::delete_tx(&mut tx, uuid, user_id)
AttachmentStore::update_post_attachments_tx(&mut tx, post_id, user_id, uuids)
AttachmentStore::remove_post_attachments_tx(&mut tx, post_id, user_id)
```

The non-`_tx` methods on each store still exist as wrappers that create their own transactions.

### Cross-Store Coordination on `Database`

When operations need to span multiple stores atomically, use coordinating methods on `Database`:

- `db.update_post_with_attachments(UpdatePostParams { ... })` - Updates post + attachment refs in one transaction
- `db.delete_post_with_attachments(uuid, user_id)` - Cleans up attachments for all descendants + deletes post in one transaction

**Rule:** API handlers should NOT contain raw SQL queries or create transactions directly. Use store methods or `Database` coordinating methods instead.

## Rust Tests

Test files located in `tests/` folder:

- **`api_tests.rs`**: User and passkey registration API tests
- **`admin_tests.rs`**: Admin dashboard API and user settings tests
- **`attachment_tests.rs`**: Attachment upload, post linking, reference counting, and cleanup tests
- **`posts_tests.rs`**: Posts CRUD, reordering, and user isolation tests
- **`token_tests.rs`**: Dual-token authentication system tests
- **`startup_tests.rs`**: Binary startup validation (JWT secret, HTTPS, base path)

### Test-Mode IP Extractor

Tests use `local_ip_extractor()` by default, which always returns `127.0.0.1` regardless of headers. This is only available in test-mode builds.

```rust
use crowchiper::cli::local_ip_extractor;

let config = ServerConfig {
    // ...
    ip_extractor: Some(local_ip_extractor()),
};
```

For tests that need to verify IP-related behavior (e.g., IP changes triggering token refresh), use `XForwardFor` extractor instead:

```rust
use crowchiper::cli::{ClientIpHeader, IpExtractor};

let config = ServerConfig {
    // ...
    ip_extractor: Some(IpExtractor::from(ClientIpHeader::XForwardFor)),
};
// Then include `x-forwarded-for` header in requests
```

### Token Tests (`tests/token_tests.rs`)
Comprehensive tests for the dual-token authentication system:
- Access token authentication and IP validation
- Refresh token flow and automatic access token renewal
- Multiple devices/sessions per user
- User isolation (can't use/revoke other users' tokens)
- Token revocation and logout
- Login flow refresh token reuse (doesn't issue new token if valid one exists)
- Token type confusion prevention (refresh can't be used as access and vice versa)
- Deactivated/deleted user handling

## Build Plugins

Vite build plugins are located in the `plugins/` folder. The main orchestrator plugin (`plugins/index.js`) coordinates all post-build processing in sequence.

### Plugin Files
- **`plugins/index.js`** - Main orchestrator that runs all plugins in correct order
- **`plugins/css-minify.js`** - CSS class name minification (extracts classes, generates short names, applies to HTML/JS)
- **`plugins/inline-iife.js`** - Compiles and inlines IIFE scripts into HTML head
- **`plugins/sri.js`** - Adds Subresource Integrity hashes to script tags
- **`plugins/html-utils.js`** - HTML processing utilities (minification, CSS inlining, testid stripping)

### Entry Chunk Size Limit
The build enforces a **50KB maximum** for the app's main entry chunk (`index-*.js`). This ensures fast initial page loads by keeping the entry point small. If the limit is exceeded, the build fails with an error.

To reduce entry chunk size:
- Use dynamic `import()` for features not needed on initial load
- Move shared utilities to `web/app/src/shared/` (keeps them in main bundle but organized)
- Heavy libraries (CodeMirror, HEIC converter) should be lazy-loaded

### Processing Order (per HTML file)
1. Check entry chunk size (app build only, fail if > 50KB)
2. Collect and inline CSS files under 20KB
3. Minify CSS class names (and update JS files with class map)
4. Inject IIFE script based on markers
5. Replace asset placeholders
6. Strip `data-testid` attributes (when not in test mode)
7. Minify HTML
8. Add SRI attributes to scripts

### CSP Hashes Output
After build, script integrity hashes are written to `dist/csp-hashes.json`:
```json
{
  "login": ["sha384-...", "sha384-..."],
  "app": ["sha384-...", "sha384-..."]
}
```

These hashes are embedded at compile time and used to generate CSP headers for HTML responses.

## Content Security Policy (CSP)

CSP headers are automatically added to all HTML responses. The headers are built at compile time from `dist/csp-hashes.json`.

### How It Works
1. Frontend build generates `dist/csp-hashes.json` with script integrity hashes
2. `build.rs` reads the hashes and builds full CSP header strings as compile-time constants
3. `src/assets.rs` serves HTML files with the pre-built CSP headers

### CSP Policy
```
default-src 'self';
script-src '<hash1>' '<hash2>' ...;
style-src 'self' 'unsafe-inline';
img-src 'self' data:;
connect-src 'self';
frame-ancestors 'none';
form-action 'self';
base-uri 'self'
```

- **script-src**: Only scripts matching the SRI hashes can execute (no `'unsafe-inline'`)
- **style-src**: Allows inline styles (needed for dynamic styling)
- **img-src**: Allows `data:` URIs for inline images
- **frame-ancestors**: Prevents clickjacking by disallowing framing
- **form-action**: Restricts form submissions to same origin

### Separate Headers per Frontend
Each frontend has different script hashes, so they get different CSP headers:
- `LOGIN_CSP_HEADER` - For `/login/*` pages
- `APP_CSP_HEADER` - For `/fiery-sparrow/*` pages
- `DASHBOARD_CSP_HEADER` - For `/dashboard/*` pages

## Frontend Asset Architecture

The app serves multiple frontends (login, app, dashboard) from embedded assets. The architecture uses a generic pattern to avoid code duplication.

### Key Types (`src/assets.rs`)

- **`FrontendConfig`** - Configuration for a single frontend (path, CSP header, processed HTML)
- **`HtmlResponder`** - Function pointer type `fn(&str, &'static str) -> Response` for HTML responses
- **`AssetsState`** - Holds all frontend configs plus auth state

### HTML Response Strategy

Instead of checking `csp_nonce` in every handler, a function pointer is chosen once at startup:
- `html_response_static` - Uses static CSP header (faster)
- `html_response_with_nonce` - Generates random nonce per request (for Cloudflare bot detection)

### Generic Handler Pattern

The `serve_frontend<T: Embed>()` function handles all frontends:
1. For HTML files: check `processed_html` first (base path rewriting), then raw assets
2. For other files: serve directly from embedded assets with appropriate caching

Individual handlers are thin wrappers that specify the Embed type and auth requirements:
```rust
pub async fn app_handler(state, AssetAuth(_), path) -> Response {
    serve_frontend::<AppAssets>(path, &state.app, state.html_responder)
}
```

### Adding a New Frontend

1. **Vite config** (`vite.config.js`): Add to `devApps` array and create build config
2. **build.rs**: Add `CONFIG_*_ASSETS` env var and CSP header generation
3. **assets.rs**: Add `#[derive(Embed)]` struct, CSP constant, and handler function
4. **lib.rs**: Create `FrontendConfig`, add routes

## Playwright E2E Tests

E2E tests located in `e2e/` folder. See `e2e/CLAUDE.md` for details.

**First-time setup**: Run `npx playwright install chromium` to download the browser.

## GitHub Actions CI/CD

Workflows located in `.github/workflows/`:

### CI Workflow (`ci.yml`)
Runs on push to `main` and pull requests. Jobs:

1. **Lint** - Runs `npm run lint:fix` and checks for uncommitted changes
2. **Rust Tests** - Builds frontend and Rust in test mode, runs `npm run test:rust`
3. **Playwright Tests** - Builds in test mode, runs Playwright tests, uploads report artifact
4. **Build Linux** - Builds production frontend and release binary, uploads as artifact

### Release Workflow (`release.yml`)
Triggered by pushing a version tag (e.g., `v1.0.0`):

1. Waits for CI to pass
2. Downloads compiled binary from CI artifacts
3. Creates tarball (`crowchiper-linux-x86_64.tar.gz`)
4. Generates release notes from conventional commits using git-cliff
5. Creates GitHub release with the binary and changelog

**Creating a release:**
```bash
git tag v1.0.0
git push origin v1.0.0
```

**Version Validation:**
The release workflow validates that the tag version matches `Cargo.toml` version before creating the release. If they don't match, the release fails.

## Version Information

Version and git commit hash are embedded at compile time and exposed via the `/api/config` endpoint and the settings menu.

### Compile-Time Embedding (`build.rs`)
- `CARGO_PKG_VERSION` - Version from Cargo.toml (built-in)
- `GIT_COMMIT_HASH` - Full git commit hash

### API Response (`GET /api/config`)
```json
{
  "no_signup": false,
  "authenticated": true,
  "version": "0.0.3",
  "git_hash": "7a90bed..."
}
```

### Frontend Display
- Settings menu shows version number as button label (e.g., "v0.0.3")
- Clicking version opens modal with version and short build hash (first 7 chars)
- Version info is fetched once on page load and cached
- Modal: `web/app/index.html` (`#version-modal`)
- Logic: `web/app/src/main.ts` (fetches from `/api/config`, app-only feature)
