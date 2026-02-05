use axum::{
    http::{StatusCode, header},
    response::{IntoResponse, Response},
};
use rust_embed::Embed;

use super::config::ProcessedHtmlMap;
use super::csp::{csp_with_nonce, generate_nonce};

/// Cache duration for immutable hashed assets (1 year)
pub const IMMUTABLE_CACHE: &str = "public, max-age=31536000, immutable";
/// Cache duration for HTML files (no cache, always revalidate)
pub const NO_CACHE: &str = "no-cache";

/// CSP header name
pub const CSP_HEADER: header::HeaderName = header::CONTENT_SECURITY_POLICY;

/// Function signature for HTML response generation.
/// Takes HTML body and CSP header, returns a Response.
pub type HtmlResponder = fn(&str, &'static str) -> Response;

/// Serve an HTML response with CSP header (no nonce)
#[inline]
pub fn html_response_static(body: &str, csp: &'static str) -> Response {
    (
        [
            (header::CONTENT_TYPE, "text/html"),
            (header::CACHE_CONTROL, NO_CACHE),
            (CSP_HEADER, csp),
        ],
        body.to_owned(),
    )
        .into_response()
}

/// Serve an HTML response with CSP header and a random nonce
#[inline]
pub fn html_response_with_nonce(body: &str, base_csp: &'static str) -> Response {
    let nonce = generate_nonce();
    let csp = csp_with_nonce(base_csp, &nonce);
    (
        [
            (header::CONTENT_TYPE, "text/html"),
            (header::CACHE_CONTROL, NO_CACHE),
        ],
        [(CSP_HEADER, csp)],
        body.to_owned(),
    )
        .into_response()
}

/// Get MIME type from file extension. Only supports types we actually serve.
#[inline]
pub fn mime_from_path(path: &str) -> &'static str {
    match path.rsplit('.').next() {
        Some("js") => "text/javascript",
        Some("css") => "text/css",
        Some("html") => "text/html",
        _ => "application/octet-stream",
    }
}

/// Serve HTML directly from embedded assets.
/// Used when no base path rewriting is needed.
#[inline]
pub fn serve_html_raw<T: Embed>(
    path: &str,
    csp_header: &'static str,
    html_responder: HtmlResponder,
) -> Response {
    if let Some(content) = T::get(path) {
        let html = String::from_utf8_lossy(&content.data);
        html_responder(&html, csp_header)
    } else {
        StatusCode::NOT_FOUND.into_response()
    }
}

/// Serve HTML from processed HTML map (for base path rewriting).
#[inline]
pub fn serve_html_processed(
    path: &str,
    processed_html: &ProcessedHtmlMap,
    csp_header: &'static str,
    html_responder: HtmlResponder,
) -> Response {
    if let Some(&html) = processed_html.get(path) {
        html_responder(html, csp_header)
    } else {
        StatusCode::NOT_FOUND.into_response()
    }
}

/// Serve a non-HTML asset from embedded files
#[inline]
pub fn serve_asset<T: Embed>(path: &str) -> Response {
    match T::get(path) {
        Some(content) => {
            let mime = mime_from_path(path);
            // Hashed assets in /assets/ are immutable, HTML files should not be cached
            let cache_control = if path.starts_with("assets/") {
                IMMUTABLE_CACHE
            } else {
                NO_CACHE
            };
            (
                [
                    (header::CONTENT_TYPE, mime),
                    (header::CACHE_CONTROL, cache_control),
                ],
                content.data,
            )
                .into_response()
        }
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

/// Normalize a path, defaulting to "index.html" if empty or missing
#[inline]
pub fn normalize_path(path: Option<&axum::extract::Path<String>>) -> &str {
    match path.map(|p| p.as_str()) {
        Some(p) if !p.is_empty() => p,
        _ => "index.html",
    }
}
