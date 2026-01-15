# Rust Backend

## Structure

- `main.rs` - CLI entry point
- `lib.rs` - Server setup (`create_app`)
- `assets.rs` - Static file serving
- `auth.rs` - JWT authentication middleware
- `jwt.rs` - JWT token generation/validation
- `api/` - API endpoints
- `db/` - Database layer

## Database

SQLite with `sqlx` async pool. Migrations in `db/mod.rs`.

**Tables**: users, passkeys, registration_challenges, posts, user_encryption_settings, attachments, post_attachments

**Access**: `db.users()`, `db.passkeys()`, `db.challenges()`, etc.

**Transactions**:
```rust
let mut tx = state.db.begin().await?;
sqlx::query("...").execute(&mut *tx).await?;
tx.commit().await?;
```

## Adding Migrations

1. Increment `CURRENT_VERSION` in `db/mod.rs`
2. Add `migrate_vN()` method
3. Call it in `migrate()` with version check

## Authentication

JWT tokens in `auth_token` cookie, 24h expiration.

Use `RequireAuth` extractor for protected routes:
```rust
pub async fn handler(auth: Result<RequireAuth, AuthError>) -> Response
```

## Error Handling

Don't leak internal errors. Use `db_err()` and `webauthn_err()` from `ResultExt` to log and return generic messages.

## WebAuthn

Uses `webauthn-rs`. CLI args: `--rp-id`, `--rp-origin`.

Challenge storage in database (persists across restarts, 5min expiry).

## Test Mode

Test-mode specific code is gated with `#[cfg(feature = "test-mode")]`. In `main.rs`:
- `test_mode::maybe_update_rp_origin()` - Updates rp_origin to include actual port when using `--port 0` with localhost (needed for WebAuthn origin validation in e2e tests)
- `CROWCHIPER_READY port=<port>` - Printed to stdout for test harnesses to capture the port
