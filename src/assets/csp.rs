use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use rand::RngCore;

/// Pre-built CSP header for login pages (built at compile time)
pub const LOGIN_CSP_HEADER: &str = env!("CSP_HEADER_LOGIN");
/// Pre-built CSP header for app pages (built at compile time)
pub const APP_CSP_HEADER: &str = env!("CSP_HEADER_APP");
/// Pre-built CSP header for dashboard pages (built at compile time)
pub const DASHBOARD_CSP_HEADER: &str = env!("CSP_HEADER_DASHBOARD");

/// Generate a random 128-bit nonce as base64
pub fn generate_nonce() -> String {
    let mut bytes = [0u8; 16];
    rand::rng().fill_bytes(&mut bytes);
    BASE64.encode(bytes)
}

/// Build a CSP header value with a nonce added to script-src
pub fn csp_with_nonce(base_csp: &str, nonce: &str) -> String {
    // Insert nonce after script-src directive
    // The base CSP has format: "... script-src 'hash1' 'hash2' ...; ..."
    // We want: "... script-src 'nonce-XXX' 'hash1' 'hash2' ...; ..."
    if let Some(pos) = base_csp.find("script-src ") {
        let insert_pos = pos + "script-src ".len();
        let nonce_value = format!("'nonce-{}' ", nonce);
        let mut result = String::with_capacity(base_csp.len() + nonce_value.len());
        result.push_str(&base_csp[..insert_pos]);
        result.push_str(&nonce_value);
        result.push_str(&base_csp[insert_pos..]);
        result
    } else {
        // Fallback: just return base CSP if script-src not found
        base_csp.to_string()
    }
}
