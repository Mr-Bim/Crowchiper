use std::fs;

fn main() {
    println!("cargo:rerun-if-changed=config.json");

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
}
