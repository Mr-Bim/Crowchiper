//! Content Security Policy middleware.
//!
//! Adds CSP headers to restrict script and style execution to trusted sources.

use axum::{
    body::Body,
    http::{Request, Response, header},
    middleware::Next,
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use rust_embed::Embed;
use std::collections::HashSet;
use std::sync::LazyLock;

use crate::assets::{AppAssets, LoginAssets};

/// Extract inline script content from HTML.
/// Looks for `<script>...</script>` tags without src attribute.
fn extract_inline_scripts(html: &str) -> Vec<String> {
    let mut scripts = Vec::new();
    let mut remaining = html;

    while let Some(start_idx) = remaining.find("<script>") {
        let after_tag = &remaining[start_idx + 8..];
        if let Some(end_idx) = after_tag.find("</script>") {
            let script_content = &after_tag[..end_idx];
            scripts.push(script_content.to_string());
            remaining = &after_tag[end_idx + 9..];
        } else {
            break;
        }
    }

    scripts
}

/// Compute SHA-256 hash of content and return base64-encoded hash for CSP.
fn compute_script_hash(content: &str) -> String {
    let hash = openssl::sha::sha256(content.as_bytes());
    let base64_hash = STANDARD.encode(hash);
    format!("'sha256-{}'", base64_hash)
}

/// Collect all unique inline script hashes from embedded HTML files.
fn collect_inline_script_hashes<T: Embed>() -> HashSet<String> {
    let mut hashes = HashSet::new();

    for file_name in T::iter() {
        if file_name.ends_with(".html") {
            if let Some(content) = T::get(&file_name) {
                let html = String::from_utf8_lossy(&content.data);
                for script in extract_inline_scripts(&html) {
                    let hash = compute_script_hash(&script);
                    hashes.insert(hash);
                }
            }
        }
    }

    hashes
}

/// Build the CSP header value at startup from embedded assets.
static CSP_HEADER_VALUE: LazyLock<String> = LazyLock::new(|| {
    let mut script_hashes = collect_inline_script_hashes::<LoginAssets>();
    script_hashes.extend(collect_inline_script_hashes::<AppAssets>());

    // Build script-src directive with 'self' and inline script hashes
    let script_src = if script_hashes.is_empty() {
        "'self'".to_string()
    } else {
        let hashes: Vec<_> = script_hashes.into_iter().collect();
        format!("'self' {}", hashes.join(" "))
    };

    // CSP directives:
    // - default-src 'self': Only allow resources from same origin by default
    // - script-src 'self' + hashes: Allow scripts from same origin and specific inline scripts
    // - style-src 'self' 'unsafe-inline': Allow styles from same origin and inline styles
    //   (inline styles are used for minified CSS and dynamic theming)
    // - img-src 'self' data: blob:: Allow images from same origin, data URIs, and blob URLs
    //   (needed for image previews and attachments)
    // - connect-src 'self': Only allow fetch/XHR to same origin
    // - frame-ancestors 'none': Prevent embedding in iframes (clickjacking protection)
    // - form-action 'self': Only allow form submissions to same origin
    // - base-uri 'self': Restrict <base> tag to same origin
    // - object-src 'none': Disallow plugins (Flash, Java, etc.)
    format!(
        "default-src 'self'; \
         script-src {}; \
         style-src 'self' 'unsafe-inline'; \
         img-src 'self' data: blob:; \
         connect-src 'self'; \
         frame-ancestors 'none'; \
         form-action 'self'; \
         base-uri 'self'; \
         object-src 'none'",
        script_src
    )
});

/// Middleware that adds Content-Security-Policy headers to responses.
pub async fn csp_middleware(request: Request<Body>, next: Next) -> Response<Body> {
    let mut response = next.run(request).await;

    // Add CSP header to all responses
    response.headers_mut().insert(
        header::CONTENT_SECURITY_POLICY,
        CSP_HEADER_VALUE
            .parse()
            .expect("CSP header value should be valid"),
    );

    // Add X-Content-Type-Options to prevent MIME sniffing
    response
        .headers_mut()
        .insert(header::X_CONTENT_TYPE_OPTIONS, "nosniff".parse().unwrap());

    // Add X-Frame-Options as fallback for older browsers
    response
        .headers_mut()
        .insert(header::X_FRAME_OPTIONS, "DENY".parse().unwrap());

    response
}
