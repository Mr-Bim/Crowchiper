# Testing

## Running Tests

```bash
npm run test:rust    # All Rust tests (requires cargo build --features test-mode)
npm run test:web     # All Playwright E2E tests (requires build:test + cargo build)
npm run test:all     # Both
cargo test --test api_tests             # Specific test file
npx playwright test                     # E2E browser tests directly
```

## Test Types

**API tests** (for backend logic):
- Use `tower::ServiceExt::oneshot` directly
- Fast, good for edge cases and API validation

**Startup tests** (for CLI validation):
- Test command-line argument handling
- Test configuration validation

**Plugin tests** (for WASM plugin system):
- Test plugin loading, sandboxing, permissions, resource limits
- Test permission isolation between plugins

**Unit tests** (for isolated logic):
- Place in `#[cfg(test)]` modules within source files

**E2E tests** (for user-facing features):
- Use Playwright in `e2e/` folder
- See `e2e/CLAUDE.md` for details

## Test Files

| File | Purpose |
|------|---------|
| `api_tests.rs` | User creation, deletion, authentication flow |
| `admin_tests.rs` | Admin dashboard and user management API |
| `attachment_tests.rs` | Image upload, reference counting, cleanup |
| `csp_nonce_tests.rs` | CSP nonce header functionality |
| `plugin_tests.rs` | WASM plugin system (40 tests) |
| `plugin_permission_isolation_tests.rs` | Plugin permission isolation (7 tests) |
| `posts_tests.rs` | Posts CRUD, encryption, reordering |
| `startup_tests.rs` | CLI validation, JWT secrets, base paths |
| `token_tests.rs` | Dual-token auth system, refresh flow |
| `common/mod.rs` | Test utilities (generate_test_key, create_test_db) |

## Checklist

- Happy path works end-to-end
- Error cases return appropriate status codes
- Works with `--base` path if applicable
