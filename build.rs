use std::fs;
use std::process::Command;

/// Format hashes from JSON array into CSP format: 'hash1' 'hash2' ...
fn format_hashes(hashes: &serde_json::Value) -> String {
    hashes
        .as_array()
        .expect("hashes must be an array")
        .iter()
        .map(|v| format!("'{}'", v.as_str().expect("hash must be a string")))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Build a CSP header from directive key-value pairs
fn build_csp(directives: &[(&str, &str)]) -> String {
    directives
        .iter()
        .map(|(key, value)| format!("{} {}", key, value))
        .collect::<Vec<_>>()
        .join("; ")
}

fn main() {
    println!("cargo:rerun-if-changed=config.json");
    println!("cargo:rerun-if-changed=dist/csp-hashes.json");
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=.git/refs/heads");

    // Embed version from Cargo.toml (already available as CARGO_PKG_VERSION)
    // Embed git commit hash
    let git_hash = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    println!("cargo:rustc-env=GIT_COMMIT_HASH={}", git_hash);

    // Get short hash for display
    let git_hash_short = if git_hash.len() >= 7 {
        &git_hash[..7]
    } else {
        &git_hash
    };
    println!("cargo:rustc-env=GIT_COMMIT_HASH_SHORT={}", git_hash_short);

    let config = fs::read_to_string("config.json").expect("Failed to read config.json");
    let json: serde_json::Value =
        serde_json::from_str(&config).expect("Failed to parse config.json");
    let assets = json["assets"].as_str().expect("assets must be a string");

    // Validate assets path format
    if !assets.starts_with('/') {
        panic!("config.json: assets must start with '/', got: {}", assets);
    }
    if assets.len() > 1 && assets.ends_with('/') {
        panic!("config.json: assets must not end with '/', got: {}", assets);
    }

    // App assets path from config.json (e.g., "/fiery-sparrow")
    println!("cargo:rustc-env=CONFIG_APP_ASSETS={}", assets);

    // Login assets path is always /login
    println!("cargo:rustc-env=CONFIG_LOGIN_ASSETS=/login");

    // Dashboard assets path is always /dashboard
    println!("cargo:rustc-env=CONFIG_DASHBOARD_ASSETS=/dashboard");

    // Load CSP hashes from build output
    let csp_hashes =
        fs::read_to_string("dist/csp-hashes.json").expect("Failed to read dist/csp-hashes.json");
    let csp_json: serde_json::Value =
        serde_json::from_str(&csp_hashes).expect("Failed to parse dist/csp-hashes.json");

    // Build CSP header for login pages
    let login_script_hashes = format_hashes(&csp_json["login"]["scripts"]);
    let login_style_hashes = format_hashes(&csp_json["login"]["styles"]);
    let login_csp = build_csp(&[
        ("default-src", "'none'"),
        (
            "script-src",
            &format!("'strict-dynamic' {}", login_script_hashes),
        ),
        ("style-src", &format!("'self' {}", login_style_hashes)),
        ("img-src", "'self' data:"),
        ("connect-src", "'self'"),
        ("frame-ancestors", "'none'"),
        ("form-action", "'self'"),
        ("base-uri", "'self'"),
    ]);
    println!("cargo:rustc-env=CSP_HEADER_LOGIN={}", login_csp);

    // Build CSP header for app pages
    // - 'strict-dynamic': allows scripts loaded by hash-validated scripts (for dynamic imports)
    // - 'unsafe-inline' in style-src: required because CodeMirror sets inline styles dynamically
    //   (hashes are incompatible with 'unsafe-inline' - browser ignores 'unsafe-inline' if hashes present)
    let app_script_hashes = format_hashes(&csp_json["app"]["scripts"]);
    let app_csp = build_csp(&[
        ("default-src", "'none'"),
        (
            "script-src",
            &format!("'strict-dynamic' {}", app_script_hashes),
        ),
        ("style-src", "'self' 'unsafe-inline'"),
        ("img-src", "'self' data: blob:"),
        ("connect-src", "'self'"),
        ("frame-ancestors", "'none'"),
        ("form-action", "'self'"),
        ("base-uri", "'self'"),
    ]);
    println!("cargo:rustc-env=CSP_HEADER_APP={}", app_csp);

    // Build CSP header for dashboard pages
    let dashboard_script_hashes = format_hashes(&csp_json["dashboard"]["scripts"]);
    let dashboard_style_hashes = format_hashes(&csp_json["dashboard"]["styles"]);
    let dashboard_csp = build_csp(&[
        ("default-src", "'none'"),
        (
            "script-src",
            &format!("'strict-dynamic' {}", dashboard_script_hashes),
        ),
        ("style-src", &format!("'self' {}", dashboard_style_hashes)),
        ("img-src", "'self' data:"),
        ("connect-src", "'self'"),
        ("frame-ancestors", "'none'"),
        ("form-action", "'self'"),
        ("base-uri", "'self'"),
    ]);
    println!("cargo:rustc-env=CSP_HEADER_DASHBOARD={}", dashboard_csp);
}
