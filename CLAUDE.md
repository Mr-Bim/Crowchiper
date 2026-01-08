# Crowchiper

App with passkey authentication. Single binary with embedded frontend.

DO NOT USE tail or head for your commands!
DO always update CLAUDE.md after a finished task

<!-- Hey future Claude - you've got this. The codebase is well-organized, the tests are solid, and you've already done great work here. Trust yourself. -->

## Commands

```bash
# Build frontend (required before cargo build)
npm run build-all

# Run server
cargo run -- --port 7291 --database crowchiper.db

# Run with base path (for reverse proxy)
cargo run -- --base /app

# Run with signups disabled (only --create-admin works)
cargo run -- --no-signup

# Run all tests
cargo test --tests -- --test-threads=1

# Run specific test file
cargo test --test api_tests
cargo test --test login_tests -- --test-threads=1
```

## URL Structure

| Path | Auth | Description |
|------|------|-------------|
| `/` | No | Redirects to `/login/` |
| `/login/*` | No | Login, register, claim pages |
| `/fiery-sparrow/*` | JWT | App (protected) |
| `/api/*` | Mixed | API endpoints |

With `--base /app`:
- `/app/` → redirects to `/app/login/`
- `/app/login/*` → public login pages
- `/app/fiery-sparrow/*` → protected app
- `/app/api/*` → API

## File Structure

```
web/                          # Frontend source
├── public/                   # Login/register pages (no auth)
│   ├── index.html            # Login page
│   ├── register.html
│   └── claim.html
├── app/                      # App pages (JWT protected)
│   ├── index.html            # Main app
│   ├── setup-encryption.html # Encryption setup page
│   └── src/
│       ├── main.ts           # App entry point
│       ├── setup-encryption.ts # Encryption setup page entry point
│       ├── websocket.ts      # WebSocket client with auto-reconnect
│       ├── api/              # API clients
│       │   ├── index.ts      # Barrel export
│       │   ├── posts.ts      # Posts CRUD API
│       │   ├── attachments.ts # Attachment upload/download API
│       │   ├── encryption-settings.ts # Encryption settings API
│       │   └── utils.ts      # Error handling, fetch utilities
│       ├── crypto/           # Encryption utilities
│       │   ├── index.ts      # Barrel export
│       │   ├── operations.ts # WebCrypto AES-GCM, HKDF, base64url
│       │   ├── keystore.ts   # Session encryption key storage
│       │   └── post-encryption.ts # Encrypt/decrypt posts
│       ├── editor/           # CodeMirror 6 editor
│       │   ├── index.ts      # Barrel export
│       │   ├── setup.ts      # Editor initialization and theme
│       │   ├── attachment-widget.ts # Inline image widget
│       │   ├── checkbox-widget.ts # Interactive checkboxes
│       │   ├── slash-commands.ts # Command palette (/heading, etc.)
│       │   └── thumbnail.ts  # Canvas thumbnail generation
│       ├── posts/            # Posts management
│       │   ├── index.ts      # Barrel export
│       │   ├── state.ts      # Centralized post/editor state
│       │   ├── ui.ts         # Post list, selection, save logic
│       │   └── drag-and-drop.ts # Post reordering
│       └── unlock/           # Unlock flow
│           └── index.ts      # Passkey unlock modal
├── inline/inline.ts          # Shared JS injected into all pages
└── styles.css                # Global styles

src/                          # Rust backend
├── main.rs                   # CLI entry point
├── lib.rs                    # Server setup (create_app)
├── assets.rs                 # Static file serving (LoginAssets, AppAssets)
├── auth.rs                   # JWT authentication middleware
├── jwt.rs                    # JWT token generation/validation
├── api/
│   ├── mod.rs                # API router
│   ├── users.rs              # User endpoints
│   ├── passkeys.rs           # Passkey registration endpoints
│   ├── posts.rs              # Posts CRUD endpoints
│   ├── encryption.rs         # Encryption setup endpoints
│   └── attachments.rs        # Image attachment endpoints
└── db/
    ├── mod.rs                # Database with migrations
    ├── user.rs               # UserStore
    ├── passkey.rs            # PasskeyStore
    ├── challenge.rs          # ChallengeStore (WebAuthn registration state)
    ├── posts.rs              # PostStore
    ├── encryption.rs         # EncryptionSettingsStore
    └── attachments.rs        # AttachmentStore (encrypted images)

tests/
├── common/mod.rs             # Test infrastructure (setup, TestContext)
├── api_tests.rs              # API tests (no browser)
├── app_auth_tests.rs         # JWT authentication tests
├── login_tests.rs            # Browser tests
├── register_tests.rs
├── claim_tests.rs            # Admin claim browser tests
├── reclaim_tests.rs          # Account reclaim tests (API + browser)
├── theme_tests.rs
├── base_path_tests.rs
├── startup_tests.rs          # CLI and startup tests
├── posts_tests.rs            # Posts API tests
├── encryption_tests.rs       # Encryption flow tests
└── attachments_tests.rs      # Attachment API tests
```

## Development instructions
Always run cargo build when you're finsihed with new functionality
Always write tests for new functionaly and run all of them before you're finsihed.
When changing frontend code, run `npm run check` to verify TypeScript types.
Page-specific CSS should be in its own file (e.g., `setup-encryption.css` for `setup-encryption.html`), not inline `<style>` blocks.
For JS-controlled visibility, use data attributes (e.g., `data-visible`) instead of CSS classes. The CSS minifier mangles class names but not data attributes, so `element.classList.add("visible")` won't work with `#foo.visible { display: block }`. Use `element.setAttribute("data-visible", "")` with `#foo[data-visible] { display: block }` instead.
 * .gl-minify-disable-NAME { --marker: 1 } turns OFF css minification
 * .gl-minify-enable-NAME { --marker: 1 } turns it back ON

**Never expose internal database IDs.** The `id` column (integer primary key) in database tables is for internal use only. Always use UUIDs (`uuid` column) when communicating with clients via API responses, WebSocket messages, or any external interface. Internal IDs can leak information about database size and ordering.

## Configuration

`config.json` contains shared settings used by both Vite and Rust:

```json
{"assets": "/fiery-sparrow"}
```

- **assets**: App asset URL prefix (JWT-protected). Must start with `/` and not end with `/`. Login pages are always at `/login`.

## Build System

Two separate Vite builds:

1. **Login build**: `web/public/` → `dist/login/` with base `/login`
2. **App build**: `web/app/` → `dist/app/` with base from `config.assets`

Build scripts:
- `npm run build-iife` - Build shared inline script
- `npm run build-login` - Build login pages
- `npm run build-app` - Build app pages
- `npm run build-all` - All of the above

Features:
- `web/inline/inline.ts` → built as IIFE, injected into all HTML
- CSS < 20KB is inlined, `styles.css` stays external
- `rust-embed` embeds `dist/login/` and `dist/app/` into binary
- `--base` CLI flag rewrites asset URLs at runtime

## Authentication

### JWT Tokens

Tokens stored in `auth_token` cookie. Generated on successful passkey login.

```rust
// Token claims
pub struct Claims {
    pub sub: String,      // User UUID
    pub username: String,
    pub role: UserRole,   // User or Admin
    pub iat: u64,         // Issued at
    pub exp: u64,         // Expiration (24h default)
}

// Generate token
let jwt = JwtConfig::new(&secret);
let token = jwt.generate_token(uuid, username, role)?;

// Validate token
let claims = jwt.validate_token(token)?;
```

### Protected Routes

App assets at `/fiery-sparrow/*` require valid JWT. Invalid/missing tokens redirect to `/login/`.

```rust
// In handlers, use RequireAuth extractor
pub async fn app_handler(
    auth: Result<RequireAuth, AuthError>,
    ...
) -> Response {
    if let Err(e) = auth {
        return e.into_response(); // Redirects to login
    }
    // Serve protected content
}
```

## Database

SQLite with `sqlx` async connection pool. Versioned migrations in `src/db/mod.rs`.

**Schema:**
```sql
users (id, uuid, username, activated, created_at)
passkeys (id, credential_id, user_id, passkey_json, created_at)
registration_challenges (id, user_uuid, challenge_json, created_at)
posts (id, uuid, user_id, title, content, created_at, updated_at)
user_encryption_settings (user_id, setup_done, prf_salt, created_at)
attachments (id, uuid, user_id, encrypted_image, encrypted_thumbnail, reference_count, ...)
post_attachments (post_id, attachment_uuid)  -- tracks which attachments belong to which post
```

**Access pattern:**
```rust
let db = Database::open("crowchiper.db").await?;
db.users().create(uuid, username).await?;
db.users().get_by_uuid(uuid).await?;
db.passkeys().add(user_id, &passkey).await?;
db.challenges().store(user_uuid, &challenge).await?;
db.challenges().take(user_uuid).await?;  // Returns and removes
```

`Database` is `Clone` and uses `SqlitePool` internally (up to 5 connections).

**Transactions:** For operations that need atomicity across multiple tables, use `db.begin()` to start a transaction:
```rust
let mut tx = state.db.begin().await?;
sqlx::query("...").execute(&mut *tx).await?;
sqlx::query("...").execute(&mut *tx).await?;
tx.commit().await?;
```

**Challenge storage:** Registration challenges are stored in the database (not in-memory) so they persist across restarts and work with multiple server instances. Challenges expire after 5 minutes.

## API

Routes mounted at `{base}/api/`.

| Method | Path | Description |
|--------|------|-------------|
| POST | /api/users | Claim username → returns `{uuid, username}` (disabled with `--no-signup`) |
| DELETE | /api/users/{uuid} | Delete user (pending: no auth; activated: self or admin) |
| POST | /api/passkeys/register/start | Start passkey registration → returns WebAuthn options |
| POST | /api/passkeys/register/finish | Complete registration with credential |
| POST | /api/passkeys/login/start | Start passkey login → returns WebAuthn options + session_id |
| POST | /api/passkeys/login/finish | Complete login with credential → sets JWT cookie |
| DELETE | /api/passkeys/login/challenge/{session_id} | Cancel pending login challenge |
| POST | /api/passkeys/claim/start | Start account reclaim → returns WebAuthn options |
| POST | /api/passkeys/claim/finish | Complete reclaim, activate user → sets JWT cookie |
| GET | /api/posts | List user's posts |
| GET | /api/posts/{id} | Get single post |
| POST | /api/posts | Create post |
| PUT | /api/posts/{id} | Update post (optional `attachment_uuids` for beacon) |
| DELETE | /api/posts/{id} | Delete post |
| GET | /api/encryption/settings | Get encryption settings (PRF salt, etc.) |
| POST | /api/encryption/setup | Initial encryption setup (stores PRF salt) |
| POST | /api/encryption/skip | Skip encryption setup (for devices without PRF) |
| POST | /api/attachments | Upload encrypted image + thumbnail |
| GET | /api/attachments/{uuid} | Get encrypted image |
| GET | /api/attachments/{uuid}/thumbnail | Get encrypted thumbnail |

**Username validation:** non-empty, max 32 chars, alphanumeric + underscore only.

**Error handling:** Don't leak internal errors. `db_err()` and `webauthn_err()` from `ResultExt` log the error and return a generic message. For 4xx errors that need logging, log manually with `error!()` before returning `ApiError`.

## WebAuthn / Passkeys

Uses `webauthn-rs` on backend, `@simplewebauthn/browser` on frontend.

**CLI args for WebAuthn:**
- `--rp-id` - Relying Party ID (domain, default: `localhost`)
- `--rp-origin` - Relying Party Origin (full URL, default: `http://localhost:7291`)

**Important:** `webauthn-rs` returns `{ publicKey: {...} }`, but `@simplewebauthn/browser` expects just the inner object. Always pass `options.publicKey` to `startRegistration()`.

**Registration flow:**
1. Claim username → get UUID
2. POST `/api/passkeys/register/start` with `{ uuid }` → get options
3. Call `startRegistration({ optionsJSON: options.publicKey })`
4. POST `/api/passkeys/register/finish` with `{ uuid, credential }`
5. User is activated, redirect to login

**Global JS variables** (set in `inline.ts`, available via `declare const`):
- `BASE_PATH` - Base path without trailing slash (e.g., `` or `/app`)
- `API_PATH` - API base path (e.g., `/api` or `/app/api`)
- `LOGIN_PATH` - Login pages path (e.g., `/login` or `/app/login`)
- `APP_PATH` - App pages path (e.g., `/fiery-sparrow`)

## Testing

Tests use `chromiumoxide` for browser automation with a shared Chrome instance per test file. WebAuthn tests use Chrome's virtual authenticator via CDP.

**Test pattern:**
```rust
#[test]
fn test_example() {
    common::runtime().block_on(async {
        let ctx = setup().await;
        ctx.goto("/").await;
        let result = ctx.eval("document.title").await;
        ctx.wait_for("someCondition", 5000).await;
        ctx.teardown().await;
    });
}
```

Uses `#[test]` + `runtime().block_on()` instead of `#[tokio::test]` to share the browser across tests.

**Virtual Authenticator:** Test setup enables a CDP virtual authenticator (`WebAuthn.enable`, `WebAuthn.addVirtualAuthenticator`) so passkey registration completes automatically without user interaction.

**Cookie isolation:** Browser tests share cookies on `localhost`. Use `ctx.new_page()` for a fresh page or clear cookies explicitly when testing unauthenticated flows.

## Adding a New Login Page

1. Create `web/public/my-page.html`
2. Run `npm run build-all`
3. Done (auto-discovered, IIFE injected, CSS inlined)

## Adding a New App Page

1. Create `web/app/my-page.html`
2. Run `npm run build-all`
3. Done (JWT-protected automatically)

## Adding a Migration

1. Increment `CURRENT_VERSION` in `src/db/mod.rs`
2. Add `migrate_vN()` method
3. Call it in `migrate()` with version check

## Adding New Features

**Always write tests for new functionality.** Choose the appropriate test type:

1. **Browser tests** (preferred for user-facing features):
   - Use `chromiumoxide` for browser automation
   - Test the full flow from frontend to backend
   - Place in appropriate test file (`login_tests.rs`, `register_tests.rs`, etc.)
   - Virtual authenticator handles passkey operations automatically

2. **API tests** (for backend logic):
   - Use `tower::ServiceExt::oneshot` to test endpoints directly
   - Faster than browser tests, good for edge cases
   - Place in `api_tests.rs` or feature-specific file

3. **Unit tests** (for isolated logic):
   - Place in `#[cfg(test)]` modules within source files
   - Good for database operations, JWT handling, etc.

**Test checklist for new features:**
- [ ] Happy path works end-to-end
- [ ] Error cases return appropriate status codes
- [ ] Works with `--base` path if applicable
- [ ] Run `cargo test --tests -- --test-threads=1` before committing

## Account Reclaim Flow

Handles users who have a passkey but aren't activated (edge case where passkey storage succeeded but activation failed).

**Flow:**
1. User tries to login → passkey auth succeeds but user not activated
2. Server returns 403 "Account not activated"
3. Frontend redirects to `/login/claim.html?reclaim=true`
4. User clicks "Use Passkey" → authenticates with existing passkey via `/api/passkeys/claim/*`
5. Server activates user and sets JWT cookie
6. User redirected to app

## End-to-End Encryption

Posts are encrypted client-side using the WebAuthn PRF extension. The server never sees plaintext content or encryption keys.

### Architecture

The encryption key is derived directly from the PRF output each session - no keys are stored on the server.

```
PRF Output (from passkey authentication)
    │
    └── HKDF-SHA256 → Encryption Key (256-bit AES)
                          │
                          └── Encrypts/decrypts post content (AES-256-GCM)
```

**Key points:**
- Encryption key is derived from PRF output using HKDF (not stored anywhere)
- PRF salt is stored server-side, used during passkey authentication
- Each session re-derives the key by authenticating with the passkey
- Users without PRF support skip encryption entirely (no fallback)
- **No recovery mechanism** - losing your passkey means losing access to encrypted data

### Crypto Operations

Located in `web/app/src/crypto/operations.ts`:

```typescript
// Derive encryption key from PRF output via HKDF
deriveEncryptionKeyFromPrf(prfOutput: ArrayBuffer): Promise<CryptoKey>

// Extract PRF output from credential response
extractPrfOutput(credential: PublicKeyCredential): ArrayBuffer | null

// Encrypt/decrypt text content (AES-256-GCM, 12-byte IV)
encryptContent(content: string, key: CryptoKey): Promise<EncryptedData>
decryptContent(ciphertext: string, iv: string, encryptionVersion: number, key: CryptoKey): Promise<string>

// Encrypt/decrypt binary data (for images)
encryptBinary(data: ArrayBuffer, key: CryptoKey): Promise<EncryptedBinaryData>
decryptBinary(ciphertext: ArrayBuffer, iv: string, key: CryptoKey): Promise<ArrayBuffer>

// Base64url utilities
toBase64Url(buffer: ArrayBuffer): string
fromBase64Url(base64url: string): ArrayBuffer
base64UrlToUint8Array(base64url: string): Uint8Array
```

**Types:**
- `EncryptedData`: `{ ciphertext: string, iv: string }` (base64url encoded)
- `EncryptedBinaryData`: `{ ciphertext: ArrayBuffer, iv: string }`

### Encrypted Content Format

Posts store encrypted content as JSON in the `content` field:

```json
{ "v": 1, "ct": "<base64url ciphertext>", "iv": "<base64url IV>" }
```

The server treats this as an opaque string - it cannot distinguish encrypted from plaintext content.

### PRF Integration

PRF extension added to WebAuthn authentication:

```typescript
// Request PRF during authentication
const credential = await startAuthentication({
  optionsJSON: options.publicKey,
});

// Extract PRF result
const prfResult = credential.clientExtensionResults?.prf?.results?.first;
```

The PRF salt is stored in `user_encryption_settings.prf_salt` and fetched before authentication.

### Onboarding Flow

New users are directed to `/fiery-sparrow/setup-encryption.html` after first login:

1. **PRF Test**: User authenticates with passkey to test PRF support
2. **If PRF supported**: Generate random PRF salt, store via `POST /api/encryption/setup`
3. **If PRF not supported**: Skip encryption via `POST /api/encryption/skip`

After setup, `setup_done` is set to true and user proceeds to the app.

### Unlock Flow

When returning to the app with encryption enabled:

1. Check if PRF salt exists but encryption key not in session
2. Show unlock modal prompting user to authenticate
3. User authenticates with passkey (PRF extension included)
4. Derive encryption key from PRF output
5. Store key in session memory

### Session Key Storage

`web/app/src/crypto/keystore.ts` provides in-memory storage:

```typescript
setSessionMek(key: CryptoKey): void   // Store derived key for session
getSessionMek(): CryptoKey | null     // Get key (null if not unlocked)
clearSessionMek(): void               // Clear on logout

setPrfSalt(salt: string): void        // Store PRF salt (base64)
getPrfSalt(): string | null           // Get PRF salt

isEncryptionEnabled(): boolean        // Check if PRF salt is set
needsUnlock(): boolean                // Has salt but no key in session
```

Key is held in memory only and cleared on logout/tab close.

### Save Behavior

Posts use a two-phase save approach to reduce server requests:

1. **Local encryption** (1 second debounce): Content is encrypted locally after 1 second of inactivity and stored in memory
2. **Server save** (60 second interval): Encrypted content is synced to server every 60 seconds if there are changes
3. **Navigation save**: Content is saved to server immediately when switching posts or creating a new post
4. **Browser close**: Uses `sendBeacon` via `PUT /api/posts/{uuid}` with pending encrypted data and attachment refs

This approach ensures:
- Encrypted content is always ready for beacon on unexpected browser close
- Server requests are minimized (not on every keystroke)
- No data loss when navigating between posts

### Browser Support

PRF extension support:
- Chrome/Edge: Full support (security keys + Google Password Manager)
- Safari 18+: iCloud Keychain passkeys only

Users without PRF support skip encryption - posts are stored as plaintext.

## Image Attachments

Images can be attached to posts with end-to-end encryption. The server stores only encrypted blobs.

### Architecture

```
User selects image → Generate thumbnail (canvas, 200px max)
    │
    ├── Encrypt image (AES-256-GCM)
    ├── Encrypt thumbnail (AES-256-GCM)
    │
    └── Upload both → Server stores with reference_count=0
        │
        └── On post save → Update refs → reference_count incremented
```

### Markdown Format

Images use a special `attachment:` URL scheme:

```markdown
![alt text](attachment:pending)    <!-- File picker shown -->
![alt text](attachment:<uuid>)     <!-- Decrypted image displayed -->
```

The `/Image` slash command inserts `![alt text](attachment:pending)`, which the widget renders as a file picker button.

### Reference Counting

Attachments use reference counting to avoid orphaned files:

- **Upload**: Creates attachment with `reference_count=0`
- **Navigate away from post**: Saves post via `PUT /api/posts/{uuid}` with `attachment_uuids` array
- **Browser close**: Uses `sendBeacon` via `PUT /api/posts/{uuid}` with `attachment_uuids` array
- **Refs update**: Computes diff (added/removed), increments/decrements counts
- **Delete post**: Decrements refs for all attachments in that post
- **Count reaches 0**: Attachment is automatically deleted

Refs are updated when navigating between posts (not on every save) to reduce API calls. Both navigation and browser close use the same post update endpoint with `attachment_uuids`.

This allows the same image to be used in multiple posts.

### Frontend Files

| File | Description |
|------|-------------|
| `web/app/src/editor/attachment-widget.ts` | CodeMirror widget for inline images |
| `web/app/src/editor/thumbnail.ts` | Canvas-based thumbnail generation |
| `web/app/src/api/attachments.ts` | API client for attachments |
| `web/app/src/crypto/operations.ts` | `encryptBinary()` / `decryptBinary()` for images |

### Backend Files

| File | Description |
|------|-------------|
| `src/api/attachments.rs` | Upload, get, refs endpoints |
| `src/db/attachments.rs` | AttachmentStore with reference counting |

### Size Limits

- **Image**: 10MB max (encrypted)
- **Thumbnail**: 100KB max (encrypted)
- **Thumbnail dimensions**: 400px max width/height

### Binary Streaming

Attachments use binary streaming instead of base64 for efficiency:

**Upload** (multipart form data):
- `image`: Binary encrypted image data
- `image_iv`: IV string
- `thumbnail`: Binary encrypted thumbnail data  
- `thumbnail_iv`: IV string
- `encryption_version`: Version number

**Download** (binary response):
- Body: Raw encrypted binary data
- `X-Encryption-IV` header: IV for decryption

### Thumbnail-First Display

The editor shows thumbnails by default for faster loading:
1. Initial load fetches only the thumbnail (smaller, faster)
2. Clicking the thumbnail opens a full-screen overlay
3. Full image is fetched and decrypted on demand
4. Both thumbnails and full images are cached separately

### Encryption

Images use the same session encryption key as posts:

```typescript
// Encrypt image
const encrypted = await encryptBinary(imageArrayBuffer, sessionKey);

// Upload using multipart form data
await uploadAttachment({
    image: encrypted.ciphertext,        // ArrayBuffer
    image_iv: encrypted.iv,             // string
    thumbnail: thumbEncrypted.ciphertext,
    thumbnail_iv: thumbEncrypted.iv,
    encryption_version: 1,
});

// Download returns binary + IV header
const response = await getAttachmentThumbnail(uuid);
const decrypted = await decryptBinary(response.data, response.iv, key);
```

The widget maintains separate caches for thumbnails and full images.

## Posts Behavior

- **Auto-create first post**: When a user logs in with no posts, a new "Untitled" post is automatically created. No empty state message is shown.
- **After deleting last post**: A new post is automatically created, ensuring the user always has at least one post.
- **After deleting a post with others remaining**: The first remaining post is automatically selected.
