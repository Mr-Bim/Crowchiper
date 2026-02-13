//! Tests for the WASM plugin system: loading, sandbox enforcement, config validation, and CLI.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use crowchiper::plugin::{PluginError, PluginRuntime};

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

/// Build a Command with standard env vars set.
fn cli_cmd() -> Command {
    let mut cmd = Command::new(cargo_bin());
    cmd.env("JWT_SECRET", "test-secret-that-is-long-enough!!")
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

    let deadline = Instant::now() + Duration::from_secs(10);
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

#[test]
fn test_good_plugin_loads_successfully() {
    let plugin = PluginRuntime::load(&wasm_path("good")).expect("good plugin should load");
    assert_eq!(plugin.name(), "good");
    assert_eq!(plugin.version(), "1.0.0");
}

#[test]
fn test_good_plugin_has_hooks() {
    let plugin = PluginRuntime::load(&wasm_path("good")).unwrap();
    assert!(
        plugin.hooks().contains("test-hook"),
        "should contain test-hook"
    );
    assert_eq!(plugin.hooks().len(), 1, "should have exactly one hook");
}

// ── Sandbox enforcement: error messages ──────────────────────────────

#[test]
fn test_fs_error_contains_file_descriptor_message() {
    let err = PluginRuntime::load(&wasm_path("fs-error")).unwrap_err();
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

#[test]
fn test_net_error_contains_unsupported_message() {
    let err = PluginRuntime::load(&wasm_path("net-error")).unwrap_err();
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

#[test]
fn test_env_error_contains_not_present_message() {
    let err = PluginRuntime::load(&wasm_path("env-error")).unwrap_err();
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

#[test]
fn test_fs_error_includes_source_location() {
    let msg = PluginRuntime::load(&wasm_path("fs-error"))
        .unwrap_err()
        .to_string();
    assert!(
        msg.contains("fs_error.rs"),
        "error should include the plugin source file location, got: {msg}"
    );
}

#[test]
fn test_net_error_includes_source_location() {
    let msg = PluginRuntime::load(&wasm_path("net-error"))
        .unwrap_err()
        .to_string();
    assert!(
        msg.contains("net_error.rs"),
        "error should include the plugin source file location, got: {msg}"
    );
}

#[test]
fn test_env_error_includes_source_location() {
    let msg = PluginRuntime::load(&wasm_path("env-error"))
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

#[test]
fn test_empty_name_plugin_is_rejected() {
    let err = PluginRuntime::load(&wasm_path("empty-name")).unwrap_err();
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

#[test]
fn test_nonexistent_file_includes_path() {
    let bad_path = Path::new("/nonexistent/plugin.wasm");
    let err = PluginRuntime::load(bad_path).unwrap_err();
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

#[test]
fn test_invalid_wasm_includes_path() {
    let tmp = std::env::temp_dir().join(format!("bad_plugin_{}.wasm", std::process::id()));
    std::fs::write(&tmp, b"this is not valid wasm").unwrap();

    let err = PluginRuntime::load(&tmp).unwrap_err();
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

#[test]
fn test_error_display_prefixes() {
    let load_msg = PluginRuntime::load(Path::new("/nope.wasm"))
        .unwrap_err()
        .to_string();
    assert!(
        load_msg.starts_with("plugin load error:"),
        "Load variant should use 'plugin load error:' prefix, got: {load_msg}"
    );

    let runtime_msg = PluginRuntime::load(&wasm_path("fs-error"))
        .unwrap_err()
        .to_string();
    assert!(
        runtime_msg.starts_with("plugin runtime error:"),
        "Runtime variant should use 'plugin runtime error:' prefix, got: {runtime_msg}"
    );

    let config_msg = PluginRuntime::load(&wasm_path("empty-name"))
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
