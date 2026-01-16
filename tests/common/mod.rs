#![allow(dead_code)]

use crowchiper::db::Database;

/// Generate a random 32-byte test encryption key as base64url.
pub fn generate_test_key() -> String {
    use base64::Engine;
    let mut key = [0u8; 32];
    rand::RngCore::fill_bytes(&mut rand::rng(), &mut key);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(key)
}

/// Helper to create a test database.
pub async fn create_test_db() -> Database {
    Database::open(":memory:")
        .await
        .expect("Failed to open test database")
}
