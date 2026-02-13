# Test WASM Plugins

Test plugins for verifying plugin sandbox error handling. All plugins are `[[example]]` targets in a single Cargo crate, compiled to `wasm32-wasip2`.

## Plugins

| Plugin | Purpose |
|--------|---------|
| `good` | Valid plugin — loads successfully |
| `fs-error` | Tries `std::fs::write` — no filesystem access |
| `net-error` | Tries `std::net::TcpStream::connect` — no network access |
| `env-error` | Tries `std::env::var("SECRET").unwrap()` — no env vars |
| `empty-name` | Returns empty plugin name — config validation error |

## Rebuilding

Requires the `wasm32-wasip2` target: `rustup target add wasm32-wasip2`

```bash
bash tests/plugins/build.sh
```

This builds all plugins and copies the `.wasm` files to `tests/plugins/wasm/`.

The compiled `.wasm` files are committed to git so tests run without extra tooling.

## Adding a New Test Plugin

1. Create `tests/plugins/examples/<name>.rs`
2. Add `wit_bindgen::generate!` with `path: "../../wit/plugin.wit"` and implement the `Guest` trait
3. Add a `[[example]]` entry in `tests/plugins/Cargo.toml`
4. Add the example name to the copy loop in `build.sh`
5. Run `bash tests/plugins/build.sh`
6. Add test cases in `tests/plugin_tests.rs`
7. Commit the new `.wasm` file
