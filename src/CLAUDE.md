# Rust Backend

## Structure

- `main.rs` - CLI entry point, plugin loading
- `lib.rs` - Server setup (`create_app`, `run_server`)
- `cli.rs` - CLI argument parsing and validation
- `jwt.rs` - JWT token generation/validation (HS256)
- `cleanup.rs` - Scheduled cleanup (expired tokens, challenges, pending users, orphaned attachments)
- `rate_limit.rs` - Per-IP rate limiting (token bucket)
- `names.rs` - Random name generator (admin users)
- `assets/` - Static file serving
- `auth/` - Authentication system (dual-token, extractors, middleware)
- `api/` - API endpoints
- `db/` - Database layer
- `plugin/` - WASM plugin system

## CLI Flags

```bash
--port <PORT>              # Port (default: 7291)
--database <PATH>          # SQLite path (default: crowchiper.db)
--base <PATH>              # Base path for reverse proxy (e.g., /app)
--rp-id <DOMAIN>           # WebAuthn Relying Party ID (default: localhost)
--rp-origin <URL>          # WebAuthn origin URL (default: http://localhost:7291)
--jwt-secret-file <PATH>   # Read JWT secret from file
--create-admin             # Create admin user and print claim URL
--no-signup                # Disable public registration
--csp-nonce                # Add random nonce to CSP headers
--log-format <FORMAT>      # pretty|json|compact (default: pretty)
--ip-header <HEADER>       # IP extraction: CFConnectingIP|XRealIp|XForwardFor|Forward
--plugin <SPEC>            # Load plugin: path.wasm[:perm1,var-key=val,...]
--plugin-error <MODE>      # abort|warn (default: abort)
```

## Database

SQLite with `sqlx` async pool (max 5 connections). Migrations in `db/mod.rs`.

**Tables**: users, passkeys, registration_challenges, login_challenges, active_tokens, posts, user_encryption_settings, attachments, post_attachments

**Stores**: `UserStore`, `PasskeyStore`, `ChallengeStore`, `LoginChallengeStore`, `TokenStore`, `PostStore`, `AttachmentStore`, `EncryptionSettingsStore`

**Access**: `db.users()`, `db.passkeys()`, `db.challenges()`, etc.

**Transactions**:
```rust
let mut tx = state.db.begin().await?;
sqlx::query("...").execute(&mut *tx).await?;
tx.commit().await?;
```

Cross-store atomic ops use `_tx` associated functions. `Database` has coordinating methods like `update_post_with_attachments()` and `delete_post_with_attachments()`. API handlers must NOT contain raw SQL.

## Adding Migrations

1. Increment `CURRENT_VERSION` in `db/mod.rs`
2. Add `migrate_vN()` method
3. Call it in `migrate()` with version check

## Authentication (Dual-Token System)

**Access Token** (5 min, stateless):
- Cookie: `access_token`
- Claims: `sub` (UUID), `username`, `role`, `ipaddr`, `iat`, `exp`
- No JTI, no database tracking

**Refresh Token** (2 weeks, database-tracked):
- Cookie: `refresh_token`
- Claims: `jti`, `sub`, `username`, `role`, `iat`, `exp`
- Tracked in `active_tokens` table for revocation

**Auth flow**: Fast path (valid access token + matching IP) or slow path (refresh token validates against DB, issues new access token).

**Cookie clearing**: `ApiAuthError` carries `secure_cookies: bool` from state. When clearing cookies on auth failure, `; Secure` is conditionally added to match how cookies were originally set.

**Login re-authentication**: On successful login, any existing refresh token (from cookie) is revoked and a new one is always issued. This prevents stale sessions from persisting.

### Auth Extractors (`auth/extractors.rs`)

- `Auth<R>` - API auth with role constraint, returns JSON errors
- `AuthWithSession<R>` - Like `Auth<R>` but includes refresh JTI
- `OptionalAuth` - Never fails, returns `Option<AuthenticatedUser>`
- `ProtectedAsset<R>` - For assets, redirects to login on failure

Role constraints: `AnyRole`, `AdminOnly`

### State Pattern

Each API module has a state struct implementing `HasAuthState`:
```rust
struct PostsState { db: Database, jwt: Arc<JwtConfig>, secure_cookies: bool, ip_extractor: Option<IpExtractor> }
impl_has_auth_state!(PostsState);
```

## Plugin System (`plugin/`)

Wasmtime + WIT Component Model. Plugins loaded at startup via `--plugin` flag.

**Modules**: `mod.rs` (bindings), `permissions.rs` (parsing/validation), `runtime.rs` (loading/execution), `error.rs`

**Permissions**: `net`, `env-<VAR_NAME>`, `fs-read=<path>`, `fs-write=<path>` (absolute paths, canonicalized). Bare `env` is rejected — must specify variable name (e.g., `env-HOME`). `net` grants full TCP/UDP access (wasmtime WASI has no per-host restriction).

**Config variables**: `var-key=value` passed as `list<tuple<string, string>>` to plugin's `config()` export

**Resource limits**: 10M fuel (CPU), 10MB memory, 512KB stack

**Load lifecycle**: Read -> Compile -> Link WASI -> Instantiate -> Configure (call `config()`) -> Validate (name non-empty)

## Rate Limiting

Per-IP token bucket. Limits: login start (10/sec), login finish (5/10sec), user create (3/min). Test mode has 1000x higher limits. If IP extraction fails (e.g., missing/malformed header when `--ip-header` is configured), the request is rejected with 403 Forbidden — never falls back to a shared bucket or socket address.

## Error Handling

Don't leak internal errors. Use `db_err()` and `webauthn_err()` from `ResultExt` to log and return generic messages.

## WebAuthn

Uses `webauthn-rs`. CLI args: `--rp-id`, `--rp-origin`. Challenge storage in database (5min expiry).

## Test Mode

Gated with `#[cfg(feature = "test-mode")]`. In `main.rs`:
- `test_mode::maybe_update_rp_origin()` - Updates rp_origin with actual port for `--port 0`
- `CROWCHIPER_READY port=<port>` - Printed to stdout for test harnesses
