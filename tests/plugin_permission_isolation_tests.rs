//! Tests that plugin permissions are isolated: granting a permission to one plugin
//! does not affect the sandbox of another plugin loaded in the same process.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

fn wasm_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/plugins/wasm")
}

fn wasm_path(name: &str) -> PathBuf {
    wasm_dir().join(format!("{name}.wasm"))
}

fn cargo_bin() -> PathBuf {
    let mut path = std::env::current_exe().unwrap();
    path.pop();
    path.pop();
    path.push("crowchiper");
    path
}

fn cli_cmd() -> Command {
    let mut cmd = Command::new(cargo_bin());
    cmd.env("JWT_SECRET", "test-secret-that-is-long-enough!!")
        .env_remove("RUST_BACKTRACE")
        .stderr(Stdio::piped())
        .stdout(Stdio::piped());
    cmd
}

fn run_cli(args: &[&str]) -> (String, String, bool) {
    let output = cli_cmd().args(args).output().expect("Failed to run binary");
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    (stdout, stderr, output.status.success())
}

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

// ── Permission isolation: env ────────────────────────────────────────

/// env-success needs `env` permission. Loading it alongside a good plugin
/// that HAS `env` permission should not leak env access to env-success
/// when env-success itself has no permissions.
#[test]
fn test_env_permission_not_shared_between_plugins() {
    // good.wasm gets env-TEST_PLUGIN_VAR, env-success.wasm gets nothing → env-success should fail
    let (stdout, _stderr, success) = run_cli(&[
        "--plugin",
        &format!(
            "{}:env-TEST_PLUGIN_VAR",
            wasm_path("good").to_str().unwrap()
        ),
        "--plugin",
        wasm_path("env-success").to_str().unwrap(),
        "--port",
        "0",
    ]);

    assert!(!success, "env-success without env permission should fail");
    assert!(
        stdout.contains("Failed to load plugin"),
        "should report plugin failure, got: {stdout}"
    );
}

/// Reversing the order: env-success first (no perms, fails in abort mode)
/// while good has env — env-success should still fail.
#[test]
fn test_env_permission_not_shared_reversed_order() {
    let (stdout, _stderr, success) = run_cli(&[
        "--plugin",
        wasm_path("env-success").to_str().unwrap(),
        "--plugin",
        &format!(
            "{}:env-TEST_PLUGIN_VAR",
            wasm_path("good").to_str().unwrap()
        ),
        "--port",
        "0",
    ]);

    assert!(!success, "env-success without env permission should fail");
    assert!(
        stdout.contains("Failed to load plugin"),
        "should report plugin failure, got: {stdout}"
    );
}

/// Both plugins get env permission — both should succeed.
#[test]
fn test_both_plugins_with_env_permission_succeed() {
    let (stdout, _stderr) = spawn_expect_listening(&[
        "--plugin",
        &format!("{}:env-HOME", wasm_path("good").to_str().unwrap()),
        "--plugin",
        &format!("{}:env-HOME", wasm_path("good").to_str().unwrap()),
        "--port",
        "0",
    ]);

    let count = stdout.matches("Plugin loaded").count();
    assert_eq!(
        count, 2,
        "both plugins should load successfully, got {count} in: {stdout}"
    );
}

// ── Permission isolation: filesystem ─────────────────────────────────

/// fs-success needs fs-write. Loading it alongside a good plugin that has
/// fs-write should not grant filesystem access to fs-success.
#[test]
fn test_fs_permission_not_shared_between_plugins() {
    let dir = std::env::temp_dir().join("crowchiper-isolation-test-fs");
    std::fs::create_dir_all(&dir).ok();

    let good_arg = format!(
        "{}:fs-write={}",
        wasm_path("good").to_str().unwrap(),
        dir.to_str().unwrap()
    );
    let (stdout, _stderr, success) = run_cli(&[
        "--plugin",
        &good_arg,
        "--plugin",
        wasm_path("fs-success").to_str().unwrap(),
        "--port",
        "0",
    ]);

    std::fs::remove_dir_all(&dir).ok();

    assert!(!success, "fs-success without fs permission should fail");
    assert!(
        stdout.contains("Failed to load plugin"),
        "should report plugin failure, got: {stdout}"
    );
}

// ── Permission isolation: network ────────────────────────────────────

/// net-success needs net permission. Loading it alongside a good plugin
/// that has net should not give net-success network access.
#[test]
fn test_net_permission_not_shared_between_plugins() {
    let (stdout, _stderr, success) = run_cli(&[
        "--plugin",
        &format!("{}:net", wasm_path("good").to_str().unwrap()),
        "--plugin",
        wasm_path("net-success").to_str().unwrap(),
        "--port",
        "0",
    ]);

    assert!(!success, "net-success without net permission should fail");
    assert!(
        stdout.contains("Failed to load plugin"),
        "should report plugin failure, got: {stdout}"
    );
}

// ── Mixed permission isolation ───────────────────────────────────────

/// One plugin gets all permissions, another gets none — the second should
/// still be sandboxed.
#[test]
fn test_full_permissions_on_one_plugin_dont_leak_to_other() {
    let dir = std::env::temp_dir().join("crowchiper-isolation-test-mixed");
    std::fs::create_dir_all(&dir).ok();

    let good_arg = format!(
        "{}:net,env-HOME,fs-write={}",
        wasm_path("good").to_str().unwrap(),
        dir.to_str().unwrap()
    );
    // fs-success has no permissions — should fail
    let (stdout, _stderr, success) = run_cli(&[
        "--plugin",
        &good_arg,
        "--plugin",
        wasm_path("fs-success").to_str().unwrap(),
        "--port",
        "0",
    ]);

    std::fs::remove_dir_all(&dir).ok();

    assert!(
        !success,
        "fs-success without permissions should fail even when another plugin has all permissions"
    );
    assert!(
        stdout.contains("Failed to load plugin"),
        "should report plugin failure, got: {stdout}"
    );
    // The good plugin should have loaded first
    assert!(
        stdout.contains("Plugin loaded"),
        "the first plugin (good) should have loaded, got: {stdout}"
    );
}

/// Warn mode: first plugin has permissions and loads, second has no
/// permissions and is skipped, server still starts.
#[test]
fn test_warn_mode_isolates_permissions_and_continues() {
    let (stdout, _stderr) = spawn_expect_listening(&[
        "--plugin",
        &format!(
            "{}:env-TEST_PLUGIN_VAR",
            wasm_path("good").to_str().unwrap()
        ),
        "--plugin",
        wasm_path("env-success").to_str().unwrap(),
        "--plugin-error",
        "warn",
        "--port",
        "0",
    ]);

    assert!(
        stdout.contains("Plugin loaded"),
        "good plugin should load, got: {stdout}"
    );
    assert!(
        stdout.contains("Failed to load plugin, skipping"),
        "env-success should be skipped with warning, got: {stdout}"
    );
}
