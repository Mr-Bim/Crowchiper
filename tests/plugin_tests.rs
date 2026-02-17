//! Tests for the WASM plugin system: loading, sandbox enforcement, config validation, and CLI.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use crowchiper::plugin::{DEFAULT_HOOK_TIMEOUT, PluginError, PluginPermission, PluginRuntime};

fn wasm_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/plugins/wasm")
}

fn wasm_path(name: &str) -> PathBuf {
    wasm_dir().join(format!("{name}.wasm"))
}

fn cargo_bin() -> PathBuf {
    let mut path = std::env::current_exe().unwrap();
    path.pop(); // Remove test binary name
    path.pop(); // Remove deps
    path.push("crowchiper");
    path
}

fn unique_db_path() -> PathBuf {
    let id = std::thread::current().id();
    std::env::temp_dir().join(format!("crowchiper-plugin-{:?}.db", id))
}

/// Build a Command with standard env vars set.
fn cli_cmd() -> Command {
    let mut cmd = Command::new(cargo_bin());
    cmd.arg("--database")
        .arg(unique_db_path())
        .env("JWT_SECRET", "test-secret-that-is-long-enough!!")
        .env_remove("RUST_BACKTRACE")
        .stderr(Stdio::piped())
        .stdout(Stdio::piped());
    cmd
}

/// Run the binary with the given args and wait for it to exit.
/// Returns (stdout, stderr, success).
fn run_cli(args: &[&str]) -> (String, String, bool) {
    let output = cli_cmd().args(args).output().expect("Failed to run binary");

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    (stdout, stderr, output.status.success())
}

/// Spawn the binary, stream stdout until "Listening" appears, then kill and return output.
/// This avoids the race condition where kill() loses buffered pipe data.
fn spawn_expect_listening(args: &[&str]) -> (String, String) {
    let mut child = cli_cmd().args(args).spawn().expect("Failed to run binary");

    let mut stdout_handle = child.stdout.take().unwrap();
    let stdout_thread = std::thread::spawn(move || {
        let mut buf = Vec::new();
        let mut byte = [0u8; 1];
        loop {
            match stdout_handle.read(&mut byte) {
                Ok(0) => break,
                Ok(_) => {
                    buf.push(byte[0]);
                    if String::from_utf8_lossy(&buf).contains("Listening") {
                        return (true, buf);
                    }
                }
                Err(_) => break,
            }
        }
        (false, buf)
    });

    // Plugin compilation in debug mode takes ~8s per plugin. When many tests run
    // concurrently, CPU contention can push this much higher.
    let deadline = Instant::now() + Duration::from_secs(120);
    loop {
        if stdout_thread.is_finished() {
            break;
        }
        if Instant::now() > deadline {
            child.kill().ok();
            child.wait().ok();
            panic!("Timed out waiting for server to print 'Listening'");
        }
        std::thread::sleep(Duration::from_millis(50));
    }

    child.kill().ok();
    child.wait().ok();

    let (found, stdout_bytes) = stdout_thread.join().unwrap();
    let stdout = String::from_utf8_lossy(&stdout_bytes).to_string();

    let mut stderr_buf = Vec::new();
    if let Some(mut se) = child.stderr.take() {
        se.read_to_end(&mut stderr_buf).ok();
    }
    let stderr = String::from_utf8_lossy(&stderr_buf).to_string();

    if !found {
        panic!("Server never printed 'Listening', stdout: {stdout}, stderr: {stderr}");
    }

    (stdout, stderr)
}

// ── Good plugin ──────────────────────────────────────────────────────

#[tokio::test]
async fn test_good_plugin_loads_successfully() {
    let plugin = PluginRuntime::load(&wasm_path("good"), &[], &[], DEFAULT_HOOK_TIMEOUT)
        .await
        .expect("good plugin should load");
    assert_eq!(plugin.name(), "good");
    assert_eq!(plugin.version(), "1.0.0");
}

#[tokio::test]
async fn test_good_plugin_has_hooks() {
    let plugin = PluginRuntime::load(&wasm_path("good"), &[], &[], DEFAULT_HOOK_TIMEOUT)
        .await
        .unwrap();
    assert_eq!(plugin.hooks().len(), 1, "should have exactly one hook");
    assert!(
        plugin.hooks().contains(&crowchiper::plugin::Hook::Server(
            crowchiper::plugin::ServerHook::IpChange
        )),
        "should contain ip-change server hook"
    );
}

// ── Sandbox enforcement: error messages ──────────────────────────────

#[tokio::test]
async fn test_fs_error_contains_file_descriptor_message() {
    let err = PluginRuntime::load(&wasm_path("fs-error"), &[], &[], DEFAULT_HOOK_TIMEOUT)
        .await
        .unwrap_err();
    assert!(
        matches!(&err, PluginError::Runtime(_)),
        "filesystem access should cause a runtime error, got: {err}"
    );
    let msg = err.to_string();
    assert!(
        msg.contains("a.txt"),
        "error should mention the file that was attempted to open, got: {msg}"
    );
    assert!(
        msg.contains("pre-opened file descriptor"),
        "error should explain sandbox denied file access, got: {msg}"
    );
    assert!(
        msg.contains("panicked at"),
        "error should include panic location, got: {msg}"
    );
}

#[tokio::test]
async fn test_net_error_contains_unsupported_message() {
    let err = PluginRuntime::load(&wasm_path("net-error"), &[], &[], DEFAULT_HOOK_TIMEOUT)
        .await
        .unwrap_err();
    assert!(
        matches!(&err, PluginError::Runtime(_)),
        "network access should cause a runtime error, got: {err}"
    );
    let msg = err.to_string();
    assert!(
        msg.contains("not supported")
            || msg.contains("Unsupported")
            || msg.contains("Permission denied"),
        "error should mention operation not supported or permission denied, got: {msg}"
    );
    assert!(
        msg.contains("panicked at"),
        "error should include panic location, got: {msg}"
    );
}

#[tokio::test]
async fn test_env_error_contains_not_present_message() {
    let err = PluginRuntime::load(&wasm_path("env-error"), &[], &[], DEFAULT_HOOK_TIMEOUT)
        .await
        .unwrap_err();
    assert!(
        matches!(&err, PluginError::Runtime(_)),
        "env access should cause a runtime error, got: {err}"
    );
    let msg = err.to_string();
    assert!(
        msg.contains("NotPresent"),
        "error should mention NotPresent for missing env var, got: {msg}"
    );
    assert!(
        msg.contains("panicked at"),
        "error should include panic location, got: {msg}"
    );
}

// ── Sandbox enforcement: source location ─────────────────────────────

#[tokio::test]
async fn test_fs_error_includes_source_location() {
    let msg = PluginRuntime::load(&wasm_path("fs-error"), &[], &[], DEFAULT_HOOK_TIMEOUT)
        .await
        .unwrap_err()
        .to_string();
    assert!(
        msg.contains("fs_error.rs"),
        "error should include the plugin source file location, got: {msg}"
    );
}

#[tokio::test]
async fn test_net_error_includes_source_location() {
    let msg = PluginRuntime::load(&wasm_path("net-error"), &[], &[], DEFAULT_HOOK_TIMEOUT)
        .await
        .unwrap_err()
        .to_string();
    assert!(
        msg.contains("net_error.rs"),
        "error should include the plugin source file location, got: {msg}"
    );
}

#[tokio::test]
async fn test_env_error_includes_source_location() {
    let msg = PluginRuntime::load(&wasm_path("env-error"), &[], &[], DEFAULT_HOOK_TIMEOUT)
        .await
        .unwrap_err()
        .to_string();
    assert!(
        msg.contains("env_error.rs"),
        "error should include the plugin source file location, got: {msg}"
    );
}

// ── Verbose mode (RUST_BACKTRACE via CLI) ────────────────────────────

#[test]
fn test_cli_verbose_fs_error_includes_backtrace_and_stderr() {
    let output = cli_cmd()
        .env("RUST_BACKTRACE", "1")
        .args([
            "--plugin",
            wasm_path("fs-error").to_str().unwrap(),
            "--port",
            "0",
        ])
        .output()
        .expect("Failed to run binary");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!output.status.success());
    assert!(
        stdout.contains("wasm backtrace"),
        "verbose error should include wasm backtrace, got: {stdout}"
    );
    assert!(
        stdout.contains("plugin stderr"),
        "verbose error should include the plugin stderr section, got: {stdout}"
    );
    assert!(
        stdout.contains("a.txt"),
        "verbose error should still mention the file, got: {stdout}"
    );
    assert!(
        stdout.contains("failed to call config()"),
        "verbose error should include the call site, got: {stdout}"
    );
}

#[test]
fn test_cli_verbose_net_error_includes_backtrace_and_stderr() {
    let output = cli_cmd()
        .env("RUST_BACKTRACE", "1")
        .args([
            "--plugin",
            wasm_path("net-error").to_str().unwrap(),
            "--port",
            "0",
        ])
        .output()
        .expect("Failed to run binary");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!output.status.success());
    assert!(
        stdout.contains("wasm backtrace"),
        "verbose error should include wasm backtrace, got: {stdout}"
    );
    assert!(
        stdout.contains("plugin stderr"),
        "verbose error should include the plugin stderr section, got: {stdout}"
    );
    assert!(
        stdout.contains("not supported") || stdout.contains("Permission denied"),
        "verbose error should mention unsupported operation or permission denied, got: {stdout}"
    );
}

// ── Clean vs verbose mode comparison ─────────────────────────────────

#[test]
fn test_cli_clean_mode_omits_backtrace() {
    // run_cli already does env_remove("RUST_BACKTRACE")
    let (stdout, _stderr, success) = run_cli(&[
        "--plugin",
        wasm_path("fs-error").to_str().unwrap(),
        "--port",
        "0",
    ]);

    assert!(!success);
    assert!(
        !stdout.contains("wasm backtrace"),
        "clean mode should not include wasm backtrace, got: {stdout}"
    );
    assert!(
        !stdout.contains("plugin stderr"),
        "clean mode should not include raw stderr dump, got: {stdout}"
    );
    assert!(
        stdout.contains("a.txt"),
        "clean mode should still contain the error detail, got: {stdout}"
    );
}

// ── Config validation ────────────────────────────────────────────────

#[tokio::test]
async fn test_empty_name_plugin_is_rejected() {
    let err = PluginRuntime::load(&wasm_path("empty-name"), &[], &[], DEFAULT_HOOK_TIMEOUT)
        .await
        .unwrap_err();
    assert!(
        matches!(&err, PluginError::InvalidConfig(_)),
        "empty name should cause an InvalidConfig error, got: {err}"
    );
    let msg = err.to_string();
    assert!(
        msg.contains("plugin config error"),
        "Display should use the config error prefix, got: {msg}"
    );
    assert!(
        msg.contains("name is empty"),
        "error should mention empty name, got: {msg}"
    );
}

// ── Load errors ──────────────────────────────────────────────────────

#[tokio::test]
async fn test_nonexistent_file_includes_path() {
    let bad_path = Path::new("/nonexistent/plugin.wasm");
    let err = PluginRuntime::load(bad_path, &[], &[], DEFAULT_HOOK_TIMEOUT)
        .await
        .unwrap_err();
    assert!(
        matches!(&err, PluginError::Load(_)),
        "missing file should cause a load error, got: {err}"
    );
    let msg = err.to_string();
    assert!(
        msg.contains("failed to read"),
        "error should mention failed to read, got: {msg}"
    );
    assert!(
        msg.contains("/nonexistent/plugin.wasm"),
        "error should include the attempted path, got: {msg}"
    );
}

#[tokio::test]
async fn test_invalid_wasm_includes_path() {
    let tmp = std::env::temp_dir().join(format!("bad_plugin_{}.wasm", std::process::id()));
    std::fs::write(&tmp, b"this is not valid wasm").unwrap();

    let err = PluginRuntime::load(&tmp, &[], &[], DEFAULT_HOOK_TIMEOUT)
        .await
        .unwrap_err();
    let msg = err.to_string();
    std::fs::remove_file(&tmp).ok();

    assert!(
        matches!(&err, PluginError::Load(_)),
        "invalid wasm should cause a load error, got: {err}"
    );
    assert!(
        msg.contains("failed to compile"),
        "error should mention failed to compile, got: {msg}"
    );
    assert!(
        msg.contains("bad_plugin_"),
        "error should include the file path, got: {msg}"
    );
}

// ── Error variant Display prefixes ───────────────────────────────────

#[tokio::test]
async fn test_error_display_prefixes() {
    let load_msg = PluginRuntime::load(Path::new("/nope.wasm"), &[], &[], DEFAULT_HOOK_TIMEOUT)
        .await
        .unwrap_err()
        .to_string();
    assert!(
        load_msg.starts_with("plugin load error:"),
        "Load variant should use 'plugin load error:' prefix, got: {load_msg}"
    );

    let runtime_msg = PluginRuntime::load(&wasm_path("fs-error"), &[], &[], DEFAULT_HOOK_TIMEOUT)
        .await
        .unwrap_err()
        .to_string();
    assert!(
        runtime_msg.starts_with("plugin runtime error:"),
        "Runtime variant should use 'plugin runtime error:' prefix, got: {runtime_msg}"
    );

    let config_msg = PluginRuntime::load(&wasm_path("empty-name"), &[], &[], DEFAULT_HOOK_TIMEOUT)
        .await
        .unwrap_err()
        .to_string();
    assert!(
        config_msg.starts_with("plugin config error:"),
        "InvalidConfig variant should use 'plugin config error:' prefix, got: {config_msg}"
    );
}

// ── CLI integration ──────────────────────────────────────────────────

#[test]
fn test_cli_plugin_good_loads() {
    let (stdout, _stderr) = spawn_expect_listening(&[
        "--plugin",
        wasm_path("good").to_str().unwrap(),
        "--port",
        "0",
    ]);
    assert!(
        stdout.contains("Plugin loaded"),
        "log should confirm plugin loaded, got: {stdout}"
    );
}

#[test]
fn test_cli_plugin_abort_mode_logs_error() {
    let (stdout, _stderr, success) = run_cli(&[
        "--plugin",
        wasm_path("fs-error").to_str().unwrap(),
        "--plugin-error",
        "abort",
        "--port",
        "0",
    ]);

    assert!(!success, "Server should exit with error in abort mode");
    assert!(
        stdout.contains("Failed to load plugin"),
        "log should say 'Failed to load plugin', got: {stdout}"
    );
    assert!(
        stdout.contains("a.txt"),
        "log should mention the file the plugin tried to open, got: {stdout}"
    );
}

#[test]
fn test_cli_plugin_warn_mode_continues_and_logs_warning() {
    let (stdout, _stderr) = spawn_expect_listening(&[
        "--plugin",
        wasm_path("fs-error").to_str().unwrap(),
        "--plugin-error",
        "warn",
        "--port",
        "0",
    ]);
    assert!(
        stdout.contains("Failed to load plugin, skipping"),
        "log should warn about skipped plugin, got: {stdout}"
    );
    assert!(
        stdout.contains("a.txt"),
        "warning log should mention the file, got: {stdout}"
    );
}

#[test]
fn test_cli_plugin_nonexistent_file_logs_path() {
    let (stdout, _stderr, success) =
        run_cli(&["--plugin", "/nonexistent/plugin.wasm", "--port", "0"]);

    assert!(
        !success,
        "Server should exit when plugin file doesn't exist"
    );
    assert!(
        stdout.contains("Failed to load plugin"),
        "log should say 'Failed to load plugin', got: {stdout}"
    );
    assert!(
        stdout.contains("/nonexistent/plugin.wasm"),
        "log should include the missing path, got: {stdout}"
    );
    assert!(
        stdout.contains("failed to read"),
        "log should mention failed to read, got: {stdout}"
    );
}

#[test]
fn test_cli_multiple_plugins_both_load() {
    let (stdout, _stderr) = spawn_expect_listening(&[
        "--plugin",
        wasm_path("good").to_str().unwrap(),
        "--plugin",
        wasm_path("good").to_str().unwrap(),
        "--port",
        "0",
    ]);
    let count = stdout.matches("Plugin loaded").count();
    assert_eq!(
        count, 2,
        "both plugins should log 'Plugin loaded', got {count} occurrences in: {stdout}"
    );
}

#[test]
fn test_cli_abort_stops_on_first_bad_plugin() {
    let (stdout, _stderr, success) = run_cli(&[
        "--plugin",
        wasm_path("empty-name").to_str().unwrap(),
        "--plugin",
        wasm_path("good").to_str().unwrap(),
        "--port",
        "0",
    ]);

    assert!(!success, "Server should exit when first plugin fails");
    assert!(
        stdout.contains("Failed to load plugin"),
        "log should report the failed plugin, got: {stdout}"
    );
    assert!(
        stdout.contains("name is empty"),
        "log should mention the config error, got: {stdout}"
    );
    assert!(
        !stdout.contains("name=good"),
        "good plugin should not have loaded after bad plugin in abort mode"
    );
}

#[test]
fn test_cli_abort_logs_plugin_path() {
    let fs_error_path = wasm_path("fs-error");
    let path_str = fs_error_path.to_str().unwrap();
    let (stdout, _stderr, success) = run_cli(&["--plugin", path_str, "--port", "0"]);

    assert!(!success);
    assert!(
        stdout.contains("fs-error.wasm"),
        "log should include the plugin file path, got: {stdout}"
    );
}

// ── Permission: filesystem ───────────────────────────────────────────

/// Create a unique temp directory for each test to avoid races when tests
/// run in parallel.
fn unique_fs_test_dir() -> PathBuf {
    let id = format!(
        "crowchiper-plugin-test-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    );
    std::env::temp_dir().join(id)
}

#[tokio::test]
async fn test_fs_success_with_write_permission() {
    let dir = unique_fs_test_dir();
    std::fs::create_dir_all(&dir).unwrap();

    // Use the canonicalized path for the config var so it matches the
    // guest-visible preopened directory (canonicalize resolves symlinks,
    // e.g. /tmp → /private/tmp on macOS).
    let canonical_dir = std::fs::canonicalize(&dir).unwrap();
    let perms = vec![PluginPermission::FsWrite(dir.clone())];
    let config = vec![(
        "path".to_string(),
        canonical_dir.to_str().unwrap().to_string(),
    )];
    let result = PluginRuntime::load(
        &wasm_path("fs-success"),
        &perms,
        &config,
        DEFAULT_HOOK_TIMEOUT,
    )
    .await;
    std::fs::remove_dir_all(&dir).ok();

    let plugin = result.expect("fs-success should load with fs-write permission");
    assert_eq!(plugin.name(), "fs-success");
}

#[tokio::test]
async fn test_fs_success_without_permission_fails() {
    let result =
        PluginRuntime::load(&wasm_path("fs-success"), &[], &[], DEFAULT_HOOK_TIMEOUT).await;
    assert!(
        result.is_err(),
        "fs-success should fail without permissions"
    );
}

#[tokio::test]
async fn test_fs_success_with_read_only_permission_fails() {
    let dir = unique_fs_test_dir();
    std::fs::create_dir_all(&dir).unwrap();

    let canonical_dir = std::fs::canonicalize(&dir).unwrap();
    let perms = vec![PluginPermission::FsRead(dir.clone())];
    let config = vec![(
        "path".to_string(),
        canonical_dir.to_str().unwrap().to_string(),
    )];
    let result = PluginRuntime::load(
        &wasm_path("fs-success"),
        &perms,
        &config,
        DEFAULT_HOOK_TIMEOUT,
    )
    .await;
    std::fs::remove_dir_all(&dir).ok();

    assert!(
        result.is_err(),
        "fs-success needs write access, read-only should fail"
    );
}

#[tokio::test]
async fn test_fs_permission_nonexistent_dir_fails_at_load() {
    let perms = vec![PluginPermission::FsRead(PathBuf::from(
        "/nonexistent/dir/for/plugin",
    ))];
    let err = PluginRuntime::load(&wasm_path("good"), &perms, &[], DEFAULT_HOOK_TIMEOUT)
        .await
        .unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("failed to resolve filesystem path"),
        "error should mention path resolution failure, got: {msg}"
    );
}

// ── Permission: environment ──────────────────────────────────────────

/// Mutex to serialize tests that mutate process-global environment variables.
/// `set_var`/`remove_var` are not thread-safe; concurrent access is UB.
static ENV_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[tokio::test]
async fn test_env_success_with_permission() {
    let _guard = ENV_MUTEX.lock().unwrap();
    // SAFETY: serialized by ENV_MUTEX — no other test mutates env vars concurrently.
    unsafe { std::env::set_var("TEST_PLUGIN_VAR", "hello") };
    let perms = vec![PluginPermission::Env("TEST_PLUGIN_VAR".to_string())];
    let plugin = PluginRuntime::load(&wasm_path("env-success"), &perms, &[], DEFAULT_HOOK_TIMEOUT)
        .await
        .expect("env-success should load with env permission");
    assert!(
        plugin.name().contains("hello"),
        "plugin name should contain the env var value, got: {}",
        plugin.name()
    );
    unsafe { std::env::remove_var("TEST_PLUGIN_VAR") };
}

#[tokio::test]
async fn test_env_with_wrong_var_name_fails() {
    let _guard = ENV_MUTEX.lock().unwrap();
    // SAFETY: serialized by ENV_MUTEX — no other test mutates env vars concurrently.
    unsafe { std::env::set_var("TEST_PLUGIN_VAR", "hello") };
    let perms = vec![PluginPermission::Env("WRONG_VAR".to_string())];
    let result =
        PluginRuntime::load(&wasm_path("env-success"), &perms, &[], DEFAULT_HOOK_TIMEOUT).await;
    unsafe { std::env::remove_var("TEST_PLUGIN_VAR") };
    assert!(
        result.is_err(),
        "env-success should fail when only WRONG_VAR is allowed"
    );
}

#[tokio::test]
async fn test_env_success_without_permission_fails() {
    let result =
        PluginRuntime::load(&wasm_path("env-success"), &[], &[], DEFAULT_HOOK_TIMEOUT).await;
    assert!(
        result.is_err(),
        "env-success should fail without env permission"
    );
}

// ── Permission: network ──────────────────────────────────────────────

#[tokio::test]
async fn test_net_success_with_permission() {
    let perms = vec![PluginPermission::Net];
    let plugin = PluginRuntime::load(&wasm_path("net-success"), &perms, &[], DEFAULT_HOOK_TIMEOUT)
        .await
        .expect("net-success should load with net permission");
    assert_eq!(plugin.name(), "net-success");
}

#[tokio::test]
async fn test_net_success_without_permission_fails() {
    let result =
        PluginRuntime::load(&wasm_path("net-success"), &[], &[], DEFAULT_HOOK_TIMEOUT).await;
    assert!(
        result.is_err(),
        "net-success should fail without net permission"
    );
}

// ── CLI: permissions ─────────────────────────────────────────────────

#[test]
fn test_cli_plugin_with_permissions_loads() {
    let (stdout, _stderr) = spawn_expect_listening(&[
        "--plugin",
        &format!("{}:net,env-HOME", wasm_path("good").to_str().unwrap()),
        "--port",
        "0",
    ]);
    assert!(
        stdout.contains("Plugin loaded"),
        "plugin should load with permissions, got: {stdout}"
    );
}

#[test]
fn test_cli_invalid_permission_rejected() {
    let (_stdout, stderr, success) = run_cli(&["--plugin", "a.wasm:bogus", "--port", "0"]);
    assert!(!success, "invalid permission should cause exit");
    assert!(
        stderr.contains("unknown permission"),
        "error should mention unknown permission, got: {stderr}"
    );
}

#[test]
fn test_cli_fs_permission_nonexistent_dir() {
    let plugin_arg = format!(
        "{}:fs-read=/nonexistent/dir/for/plugin",
        wasm_path("good").to_str().unwrap()
    );
    let (stdout, _stderr, success) = run_cli(&["--plugin", &plugin_arg, "--port", "0"]);
    assert!(!success, "nonexistent fs path should cause load failure");
    assert!(
        stdout.contains("failed to resolve filesystem path"),
        "error should mention path resolution failure, got: {stdout}"
    );
}

// ── Resource exhaustion ──────────────────────────────────────────────

#[tokio::test]
async fn test_infinite_loop_exhausts_fuel() {
    let err = PluginRuntime::load(&wasm_path("infinite-loop"), &[], &[], DEFAULT_HOOK_TIMEOUT)
        .await
        .unwrap_err();
    assert!(
        matches!(&err, PluginError::Runtime(_)),
        "infinite loop should cause a runtime error, got: {err}"
    );
    // Wasmtime traps with "all fuel consumed" or similar when fuel runs out.
    // The error passes through our generic "failed to call config()" handler.
    let msg = err.to_string();
    assert!(
        msg.contains("fuel") || msg.contains("wasm backtrace"),
        "error should indicate execution was interrupted, got: {msg}"
    );
}

#[tokio::test]
async fn test_memory_hog_hits_limit() {
    let err = PluginRuntime::load(&wasm_path("memory-hog"), &[], &[], DEFAULT_HOOK_TIMEOUT)
        .await
        .unwrap_err();
    assert!(
        matches!(&err, PluginError::Runtime(_)),
        "memory hog should cause a runtime error, got: {err}"
    );
}

#[tokio::test]
async fn test_stack_overflow_is_caught() {
    let err = PluginRuntime::load(&wasm_path("stack-overflow"), &[], &[], DEFAULT_HOOK_TIMEOUT)
        .await
        .unwrap_err();
    assert!(
        matches!(&err, PluginError::Runtime(_)),
        "stack overflow should cause a runtime error, got: {err}"
    );
}

// ── Path validation ──────────────────────────────────────────────────

#[test]
fn test_relative_path_fs_read_rejected() {
    let err = crowchiper::plugin::parse_plugin_spec("a.wasm:fs-read=../etc").unwrap_err();
    assert!(
        err.contains("absolute path"),
        "error should mention absolute path requirement, got: {err}"
    );
}

#[test]
fn test_relative_path_fs_write_rejected() {
    let err = crowchiper::plugin::parse_plugin_spec("a.wasm:fs-write=data/subdir").unwrap_err();
    assert!(
        err.contains("absolute path"),
        "error should mention absolute path requirement, got: {err}"
    );
}

#[test]
fn test_cli_relative_path_rejected() {
    let (_stdout, stderr, success) = run_cli(&["--plugin", "a.wasm:fs-read=../etc", "--port", "0"]);
    assert!(!success, "relative path should be rejected");
    assert!(
        stderr.contains("absolute path"),
        "error should mention absolute path requirement, got: {stderr}"
    );
}

// ── Hook system ──────────────────────────────────────────────────────

use crowchiper::plugin::{Hook, PluginManager, ServerHook};

#[tokio::test]
async fn test_call_hook_invokes_registered_plugin() {
    let plugin = PluginRuntime::load(&wasm_path("hook-echo"), &[], &[], DEFAULT_HOOK_TIMEOUT)
        .await
        .unwrap();
    assert_eq!(plugin.name(), "hook-echo");

    let event = crowchiper::plugin::HookEvent {
        hook: Hook::Server(ServerHook::IpChange),
        time: 1234567890,
        target: crowchiper::plugin::HookTarget::Server,
        values: vec![
            ("old_ip".into(), "1.2.3.4".into()),
            ("new_ip".into(), "5.6.7.8".into()),
            ("user_uuid".into(), "test-uuid".into()),
        ],
    };
    let result = plugin.call_hook(&event).await;
    assert!(result.is_ok(), "hook-echo should succeed, got: {result:?}");
}

#[tokio::test]
async fn test_call_hook_returns_plugin_error() {
    let plugin = PluginRuntime::load(&wasm_path("hook-error"), &[], &[], DEFAULT_HOOK_TIMEOUT)
        .await
        .unwrap();
    assert_eq!(plugin.name(), "hook-error");

    let event = crowchiper::plugin::HookEvent {
        hook: Hook::Server(ServerHook::IpChange),
        time: 0,
        target: crowchiper::plugin::HookTarget::Server,
        values: vec![],
    };
    let result = plugin.call_hook(&event).await;
    assert!(result.is_err(), "hook-error should return an error");
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("test hook error"),
        "error should contain the plugin's error message, got: {msg}"
    );
}

#[tokio::test]
async fn test_fire_hook_skips_unregistered_plugin() {
    // Verify PluginManager doesn't panic with no matching plugins
    let manager = PluginManager::new(vec![]);
    manager
        .fire_hook(
            Hook::Server(ServerHook::IpChange),
            vec![("old_ip".into(), "1.1.1.1".into())],
        )
        .await;
    // No panic = success
}

#[tokio::test]
async fn test_fire_hook_logs_error_but_does_not_panic() {
    let plugin = PluginRuntime::load(&wasm_path("hook-error"), &[], &[], DEFAULT_HOOK_TIMEOUT)
        .await
        .unwrap();
    let manager = PluginManager::new(vec![plugin]);
    // Should not panic even though the plugin returns an error
    manager
        .fire_hook(
            Hook::Server(ServerHook::IpChange),
            vec![
                ("old_ip".into(), "1.2.3.4".into()),
                ("new_ip".into(), "5.6.7.8".into()),
                ("user_uuid".into(), "test-uuid".into()),
            ],
        )
        .await;
}

#[tokio::test]
async fn test_call_hook_can_be_called_multiple_times() {
    let plugin = PluginRuntime::load(&wasm_path("hook-echo"), &[], &[], DEFAULT_HOOK_TIMEOUT)
        .await
        .unwrap();
    let event = crowchiper::plugin::HookEvent {
        hook: Hook::Server(ServerHook::IpChange),
        time: 0,
        target: crowchiper::plugin::HookTarget::Server,
        values: vec![],
    };
    // Call multiple times to verify store is properly reusable
    for _ in 0..5 {
        plugin
            .call_hook(&event)
            .await
            .expect("repeated hook call should succeed");
    }
}

// ── Plugin host log import ───────────────────────────────────────────

#[tokio::test]
async fn test_log_plugin_loads_and_hooks() {
    let plugin = PluginRuntime::load(&wasm_path("log-test"), &[], &[], DEFAULT_HOOK_TIMEOUT)
        .await
        .unwrap();
    assert_eq!(plugin.name(), "log-test");

    let event = crowchiper::plugin::HookEvent {
        hook: Hook::Server(ServerHook::IpChange),
        time: 0,
        target: crowchiper::plugin::HookTarget::Server,
        values: vec![],
    };
    let result = plugin.call_hook(&event).await;
    assert!(
        result.is_ok(),
        "log-test hook should succeed, got: {result:?}"
    );
}

// ── Wall-clock timeout ────────────────────────────────────────────────

#[tokio::test]
async fn test_slow_hook_times_out() {
    // slow-hook loads fast (config returns immediately) but sleeps 60s in on_hook().
    // The default 5s timeout should terminate it well before 60s.
    let timeout = Duration::from_millis(50);
    let plugin = PluginRuntime::load(&wasm_path("slow-hook"), &[], &[], timeout)
        .await
        .expect("slow-hook should load (config is fast)");

    let event = crowchiper::plugin::HookEvent {
        hook: Hook::Server(ServerHook::IpChange),
        time: 0,
        target: crowchiper::plugin::HookTarget::Server,
        values: vec![],
    };

    let start = Instant::now();
    let err = plugin.call_hook(&event).await.unwrap_err();
    let elapsed = start.elapsed();

    assert!(
        matches!(&err, PluginError::Hook(_)),
        "slow hook should produce a Hook error, got: {err}"
    );
    assert!(
        err.to_string().contains("timed out"),
        "error should mention timeout, got: {err}"
    );
    assert!(
        elapsed < Duration::from_secs(10),
        "should complete in ~2s, not 60s, took: {elapsed:?}"
    );

    // After a timeout, the plugin reloads from disk. The next call should
    // also time out (the plugin still sleeps 60s), but the reload itself works.
    let err2 = plugin.call_hook(&event).await.unwrap_err();
    assert!(
        err2.to_string().contains("timed out"),
        "second call should also time out (plugin reloads but still sleeps), got: {err2}"
    );
}

#[test]
fn test_cli_log_plugin_output_appears_in_server_log() {
    let (stdout, _stderr) = spawn_expect_listening(&[
        "--plugin",
        wasm_path("log-test").to_str().unwrap(),
        "--port",
        "0",
    ]);
    assert!(
        stdout.contains("config called"),
        "server log should contain plugin log output from config(), got: {stdout}"
    );
}
