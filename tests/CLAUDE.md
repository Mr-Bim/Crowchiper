# Testing

## Running Tests

```bash
cargo test --tests -- --test-threads=1  # All tests
cargo test --test api_tests             # Specific file
```

## Test Types

**Browser tests** (preferred for user-facing features):
- Use `chromiumoxide` for browser automation
- Virtual authenticator handles passkey operations automatically

**API tests** (for backend logic):
- Use `tower::ServiceExt::oneshot` directly
- Faster, good for edge cases

**Unit tests** (for isolated logic):
- Place in `#[cfg(test)]` modules within source files

## Test Pattern

Use `#[test]` + `runtime().block_on()` (not `#[tokio::test]`) to share browser:

```rust
#[test]
fn test_example() {
    common::runtime().block_on(async {
        let ctx = setup().await;
        ctx.goto("/").await;
        ctx.wait_for("condition", 5000).await;
        ctx.teardown().await;
    });
}
```

## Cookie Isolation

Browser tests share cookies on localhost. Use `ctx.new_page()` for fresh page or clear cookies explicitly.

## Checklist

- Happy path works end-to-end
- Error cases return appropriate status codes
- Works with `--base` path if applicable
