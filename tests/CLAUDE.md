# Testing

## Running Tests

```bash
cargo test --tests -- --test-threads=1  # All tests
cargo test --test api_tests             # Specific file
npx playwright test                     # E2E browser tests
```

## Test Types

**E2E tests** (for user-facing features):
- Use Playwright in `e2e/` folder
- See `e2e/CLAUDE.md` for details

**API tests** (for backend logic):
- Use `tower::ServiceExt::oneshot` directly
- Fast, good for edge cases and API validation

**Startup tests** (for CLI validation):
- Test command-line argument handling
- Test configuration validation

**Unit tests** (for isolated logic):
- Place in `#[cfg(test)]` modules within source files

## Checklist

- Happy path works end-to-end
- Error cases return appropriate status codes
- Works with `--base` path if applicable
