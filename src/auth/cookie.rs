//! Cookie parsing utilities for authentication.

use axum::http::header;

/// Cookie name for the access token (short-lived, 5 minutes).
pub const ACCESS_COOKIE_NAME: &str = "access_token";

/// Cookie name for the refresh token (long-lived, 2 weeks).
pub const REFRESH_COOKIE_NAME: &str = "refresh_token";

/// Extract a cookie value from the Cookie header.
pub fn get_cookie<'a>(headers: &'a axum::http::HeaderMap, name: &str) -> Option<&'a str> {
    let cookie_header = headers.get(header::COOKIE)?.to_str().ok()?;
    for part in cookie_header.split(';') {
        let part = part.trim();
        if let Some((key, value)) = part.split_once('=') {
            if key.trim() == name {
                return Some(value.trim());
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    #[test]
    fn test_get_cookie_simple() {
        let mut headers = axum::http::HeaderMap::new();
        headers.insert(
            header::COOKIE,
            HeaderValue::from_static("access_token=abc123"),
        );

        assert_eq!(get_cookie(&headers, "access_token"), Some("abc123"));
    }

    #[test]
    fn test_get_cookie_multiple() {
        let mut headers = axum::http::HeaderMap::new();
        headers.insert(
            header::COOKIE,
            HeaderValue::from_static("foo=bar; access_token=abc123; refresh_token=xyz789"),
        );

        assert_eq!(get_cookie(&headers, "access_token"), Some("abc123"));
        assert_eq!(get_cookie(&headers, "refresh_token"), Some("xyz789"));
        assert_eq!(get_cookie(&headers, "foo"), Some("bar"));
    }

    #[test]
    fn test_get_cookie_not_found() {
        let mut headers = axum::http::HeaderMap::new();
        headers.insert(header::COOKIE, HeaderValue::from_static("foo=bar"));

        assert_eq!(get_cookie(&headers, "access_token"), None);
    }

    #[test]
    fn test_get_cookie_no_header() {
        let headers = axum::http::HeaderMap::new();
        assert_eq!(get_cookie(&headers, "access_token"), None);
    }

    #[test]
    fn test_get_cookie_with_spaces() {
        let mut headers = axum::http::HeaderMap::new();
        headers.insert(
            header::COOKIE,
            HeaderValue::from_static("  access_token = abc123  ; foo=bar"),
        );

        assert_eq!(get_cookie(&headers, "access_token"), Some("abc123"));
    }
}
