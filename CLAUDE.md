# Crowchiper

App with passkey authentication. Single binary with embedded frontend.

DO NOT USE tail or head for your commands.
DO always update the appropriate CLAUDE.md after a finished task.

## Commands

```bash
npm run build-all          # Build frontend for production (no test mode)
npm run build-all-test     # Build frontend with test mode
npm run lint:fix           # TypeScript type check (tsc) and lint fix (oxlint)
npm run test:rust          # Run Rust tests (requires prior cargo build --features test-mode)
npm run test:web           # Run Playwright tests (requires prior build-all-test + cargo build)
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
- JS: Use `npm run build-all-test` to build with TEST_MODE (includes test code)
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

Always import and reuse these patterns instead of defining new regex for galleries.

## Save Button

The app header includes a Save button that:
- Shows "Saved" (disabled) when there are no unsaved changes
- Shows "Save" (clickable, highlighted) when there are unsaved changes
- Uses `data-dirty` attribute for styling (`data-dirty="true"` or `data-dirty="false"`)
- Located in `web/app/index.html`, styled in `web/app/css/app.css`
- Functionality in `web/app/src/posts/ui.ts` (`handleSave`, `updateSaveButton`)

## Nested Posts (Hierarchical Structure)

Posts support unlimited nesting depth, similar to Notion:

### Database Schema
- `parent_id`: References parent post's UUID (NULL = root level)
- `is_folder`: Boolean flag (folders are not editable in editor)
- Positions are scoped per-parent (siblings ordered 0, 1, 2...)
- `ON DELETE CASCADE`: Deleting a parent deletes all children

### API Endpoints
- `GET /posts` - Returns tree structure (1 level deep by default)
- `GET /posts/{uuid}/children` - Lazy load children beyond initial depth
- `POST /posts` - Accepts `parent_id` and `is_folder` fields
- `POST /posts/{uuid}/move` - Move post to new parent: `{ parent_id, position }`
- `POST /posts/reorder` - Reorder siblings: `{ parent_id, uuids }`
- `DELETE /posts/{uuid}` - Returns `{ deleted, children_deleted }` count

### Frontend Types (`web/app/src/api/posts.ts`)
```typescript
interface PostNode {
  uuid: string;
  title: string | null;
  parent_id: string | null;
  is_folder: boolean;
  has_children: boolean;
  children: PostNode[] | null;  // null = not yet loaded
  // ... other fields
}
```

### State Management (`web/app/src/posts/state.ts`)
- `posts` is now a tree structure (`PostNode[]`)
- `expandedPosts: Set<string>` tracks expanded posts
- Helper functions: `findPost()`, `findParent()`, `getPath()`, `movePostInTree()`

### UI Behavior (`web/app/src/posts/ui.ts`)
- Tree rendered with indentation (16px per level)
- Expand/collapse chevrons for posts with children
- Folder icons for `is_folder = true`
- Click folder to expand/collapse (not edit)
- Click post to select for editing
- First 3 levels expanded by default

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
- Theme selector dropdown
- Logout button (calls `/api/tokens/logout`, redirects to login)

Located in `web/app/index.html`, styled in `web/app/css/app.css`, logic in `web/inline/inline.ts`.

### Token Cleanup
Expired tokens are deleted on server startup via `db.tokens().delete_expired()`.

## Rust Tests

Test files located in `tests/` folder:

- **`api_tests.rs`**: User and passkey registration API tests
- **`posts_tests.rs`**: Posts CRUD, reordering, and user isolation tests
- **`token_tests.rs`**: Dual-token authentication system tests
- **`startup_tests.rs`**: Binary startup validation (JWT secret, HTTPS, base path)

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

## Playwright E2E Tests

E2E tests located in `e2e/` folder. See `e2e/CLAUDE.md` for details.

**First-time setup**: Run `npx playwright install chromium` to download the browser.
