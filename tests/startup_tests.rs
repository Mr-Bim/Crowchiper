//! Tests for main.rs startup validation (JWT_SECRET, secure cookies, base paths, etc.)

use std::fs;
use std::process::{Command, Stdio};
use std::time::Duration;

fn cargo_bin() -> std::path::PathBuf {
    // Get the path to the compiled binary
    let mut path = std::env::current_exe().unwrap();
    path.pop(); // Remove test binary name
    path.pop(); // Remove deps
    path.push("crowchiper");
    path
}

#[test]
fn test_missing_jwt_secret_exits_with_error() {
    let output = Command::new(cargo_bin())
        .env_remove("JWT_SECRET")
        .stderr(Stdio::piped())
        .stdout(Stdio::piped())
        .output()
        .expect("Failed to run binary");

    assert!(
        !output.status.success(),
        "Should exit with error when JWT_SECRET is missing"
    );

    // tracing logs to stdout by default
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{}{}", stdout, stderr);
    assert!(
        combined.contains("JWT_SECRET") && combined.contains("required"),
        "Should mention JWT_SECRET is required, got: {}",
        combined
    );
}

#[test]
fn test_http_non_localhost_exits_with_error() {
    let output = Command::new(cargo_bin())
        .env("JWT_SECRET", "test-secret-that-is-long-enough!!")
        .args(["--rp-origin", "http://example.com"])
        .stderr(Stdio::piped())
        .stdout(Stdio::piped())
        .output()
        .expect("Failed to run binary");

    assert!(
        !output.status.success(),
        "Should exit with error when using HTTP for non-localhost"
    );

    // tracing logs to stdout by default
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{}{}", stdout, stderr);
    assert!(
        combined.contains("HTTPS"),
        "Should mention HTTPS requirement, got: {}",
        combined
    );
}

#[test]
fn test_http_localhost_is_allowed() {
    // Start the server and kill it quickly - we just want to verify it starts
    let mut child = Command::new(cargo_bin())
        .env("JWT_SECRET", "test-secret-that-is-long-enough!!")
        .args(["--rp-origin", "http://localhost:9999", "--port", "0"])
        .stderr(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("Failed to run binary");

    // Give it a moment to start or fail
    std::thread::sleep(Duration::from_millis(500));

    // Check if it's still running (meaning it passed validation)
    match child.try_wait() {
        Ok(Some(status)) => {
            // Process exited - check if it was an error
            let output = child.wait_with_output().unwrap();
            let stderr = String::from_utf8_lossy(&output.stderr);
            panic!(
                "Server exited unexpectedly with status {:?}, stderr: {}",
                status, stderr
            );
        }
        Ok(None) => {
            // Still running - good! Kill it.
            child.kill().ok();
        }
        Err(e) => {
            panic!("Error checking process status: {}", e);
        }
    }
}

#[test]
fn test_https_non_localhost_is_allowed() {
    // This will fail to bind but should pass the validation checks
    // We use a high port that's unlikely to be available to cause a quick failure
    // after validation passes
    let mut child = Command::new(cargo_bin())
        .env("JWT_SECRET", "test-secret-that-is-long-enough!!")
        .args([
            "--rp-origin",
            "https://example.com",
            "--rp-id",
            "example.com",
            "--port",
            "0",
        ])
        .stderr(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("Failed to run binary");

    // Give it a moment to start or fail
    std::thread::sleep(Duration::from_millis(500));

    // Check if it's still running (meaning it passed validation)
    match child.try_wait() {
        Ok(Some(_status)) => {
            let output = child.wait_with_output().unwrap();
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            let combined = format!("{}{}", stdout, stderr);
            // If it failed, make sure it wasn't due to HTTPS validation
            assert!(
                !combined.contains("HTTPS"),
                "Should not fail due to HTTPS check for https:// origin, output: {}",
                combined
            );
        }
        Ok(None) => {
            // Still running - good! Kill it.
            child.kill().ok();
        }
        Err(e) => {
            panic!("Error checking process status: {}", e);
        }
    }
}

#[test]
fn test_jwt_secret_env_is_cleared() {
    // This is harder to test from outside, but we can at least verify
    // the server starts and the env var handling doesn't crash
    let mut child = Command::new(cargo_bin())
        .env("JWT_SECRET", "test-secret-to-be-cleared-with-32-chars")
        .args(["--port", "0"])
        .stderr(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("Failed to run binary");

    std::thread::sleep(Duration::from_millis(500));

    match child.try_wait() {
        Ok(Some(status)) => {
            let output = child.wait_with_output().unwrap();
            let stderr = String::from_utf8_lossy(&output.stderr);
            panic!(
                "Server exited unexpectedly with status {:?}, stderr: {}",
                status, stderr
            );
        }
        Ok(None) => {
            child.kill().ok();
        }
        Err(e) => {
            panic!("Error checking process status: {}", e);
        }
    }
}

#[test]
fn test_jwt_secret_file() {
    // Create a temp file with a secret
    let temp_dir = std::env::temp_dir();
    let secret_file = temp_dir.join(format!("jwt_secret_test_{}", std::process::id()));
    fs::write(&secret_file, "this-is-a-long-secret-from-file-for-testing").unwrap();

    let mut child = Command::new(cargo_bin())
        .env_remove("JWT_SECRET")
        .args([
            "--jwt-secret-file",
            secret_file.to_str().unwrap(),
            "--port",
            "0",
        ])
        .stderr(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("Failed to run binary");

    std::thread::sleep(Duration::from_millis(500));

    // Clean up
    let _ = fs::remove_file(&secret_file);

    match child.try_wait() {
        Ok(Some(status)) => {
            let output = child.wait_with_output().unwrap();
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            panic!(
                "Server exited unexpectedly with status {:?}, output: {}{}",
                status, stdout, stderr
            );
        }
        Ok(None) => {
            // Still running - good! Kill it.
            child.kill().ok();
        }
        Err(e) => {
            panic!("Error checking process status: {}", e);
        }
    }
}

#[test]
fn test_jwt_secret_file_not_found() {
    let output = Command::new(cargo_bin())
        .env_remove("JWT_SECRET")
        .args(["--jwt-secret-file", "/nonexistent/path/to/secret"])
        .stderr(Stdio::piped())
        .stdout(Stdio::piped())
        .output()
        .expect("Failed to run binary");

    assert!(
        !output.status.success(),
        "Should exit with error when secret file not found"
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{}{}", stdout, stderr);
    assert!(
        combined.contains("Failed to read JWT secret file"),
        "Should mention failed to read file, got: {}",
        combined
    );
}

#[test]
fn test_jwt_secret_env_takes_precedence_over_file() {
    // Create a temp file with a different secret
    let temp_dir = std::env::temp_dir();
    let secret_file = temp_dir.join(format!("jwt_secret_precedence_{}", std::process::id()));
    fs::write(&secret_file, "file-secret").unwrap();

    // Server should start with env var even if file is also provided
    let mut child = Command::new(cargo_bin())
        .env("JWT_SECRET", "env-secret-that-is-long-enough-32chars")
        .args([
            "--jwt-secret-file",
            secret_file.to_str().unwrap(),
            "--port",
            "0",
        ])
        .stderr(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("Failed to run binary");

    std::thread::sleep(Duration::from_millis(500));

    let _ = fs::remove_file(&secret_file);

    match child.try_wait() {
        Ok(Some(status)) => {
            let output = child.wait_with_output().unwrap();
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            panic!(
                "Server exited unexpectedly with status {:?}, output: {}{}",
                status, stdout, stderr
            );
        }
        Ok(None) => {
            child.kill().ok();
        }
        Err(e) => {
            panic!("Error checking process status: {}", e);
        }
    }
}

#[test]
fn test_short_jwt_secret_exits_with_error() {
    let output = Command::new(cargo_bin())
        .env("JWT_SECRET", "short") // Less than 32 chars
        .args(["--port", "0"])
        .stderr(Stdio::piped())
        .stdout(Stdio::piped())
        .output()
        .expect("Failed to run binary");

    assert!(
        !output.status.success(),
        "Should exit with error when JWT secret is too short"
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{}{}", stdout, stderr);
    assert!(
        combined.contains("shorter than") || combined.contains("32"),
        "Should mention minimum length requirement, got: {}",
        combined
    );
}

#[test]
fn test_invalid_rp_origin_url() {
    let output = Command::new(cargo_bin())
        .env("JWT_SECRET", "test-secret-that-is-long-enough!!")
        .args(["--rp-origin", "not-a-valid-url"])
        .stderr(Stdio::piped())
        .stdout(Stdio::piped())
        .output()
        .expect("Failed to run binary");

    assert!(
        !output.status.success(),
        "Should exit with error for invalid rp-origin URL"
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{}{}", stdout, stderr);
    assert!(
        combined.contains("Invalid rp-origin"),
        "Should mention invalid rp-origin, got: {}",
        combined
    );
}

#[test]
fn test_invalid_base_path_no_leading_slash() {
    let output = Command::new(cargo_bin())
        .env("JWT_SECRET", "test-secret-that-is-long-enough!!")
        .args(["--base", "no-slash"])
        .stderr(Stdio::piped())
        .stdout(Stdio::piped())
        .output()
        .expect("Failed to run binary");

    assert!(
        !output.status.success(),
        "Should exit with error for base path without leading slash"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("must start with '/'"),
        "Should mention base path must start with /, got: {}",
        stderr
    );
}

#[test]
fn test_invalid_base_path_trailing_slash() {
    let output = Command::new(cargo_bin())
        .env("JWT_SECRET", "test-secret-that-is-long-enough!!")
        .args(["--base", "/app/"])
        .stderr(Stdio::piped())
        .stdout(Stdio::piped())
        .output()
        .expect("Failed to run binary");

    assert!(
        !output.status.success(),
        "Should exit with error for base path with trailing slash"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("must not end with '/'"),
        "Should mention base path must not end with /, got: {}",
        stderr
    );
}

#[test]
fn test_valid_base_path() {
    let mut child = Command::new(cargo_bin())
        .env("JWT_SECRET", "test-secret-that-is-long-enough!!")
        .args(["--base", "/myapp", "--port", "0"])
        .stderr(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("Failed to run binary");

    std::thread::sleep(Duration::from_millis(500));

    match child.try_wait() {
        Ok(Some(status)) => {
            let output = child.wait_with_output().unwrap();
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            panic!(
                "Server exited unexpectedly with status {:?}, output: {}{}",
                status, stdout, stderr
            );
        }
        Ok(None) => {
            child.kill().ok();
        }
        Err(e) => {
            panic!("Error checking process status: {}", e);
        }
    }
}
