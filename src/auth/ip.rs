//! Client IP extraction utilities.

use std::net::SocketAddr;

use axum::{extract::ConnectInfo, http::request::Parts};

use crate::cli::IpExtractor;

/// Trait for types that provide access to HTTP headers and extensions.
/// Implemented for both `Parts` and `Request` to allow flexible IP extraction.
pub trait HasHeadersAndExtensions {
    fn headers(&self) -> &axum::http::HeaderMap;
    fn extensions(&self) -> &axum::http::Extensions;
}

impl HasHeadersAndExtensions for Parts {
    fn headers(&self) -> &axum::http::HeaderMap {
        &self.headers
    }
    fn extensions(&self) -> &axum::http::Extensions {
        &self.extensions
    }
}

impl<B> HasHeadersAndExtensions for axum::extract::Request<B> {
    fn headers(&self) -> &axum::http::HeaderMap {
        axum::extract::Request::headers(self)
    }
    fn extensions(&self) -> &axum::http::Extensions {
        axum::extract::Request::extensions(self)
    }
}

/// Extract client IP address based on configuration.
///
/// If `ip_extractor` is set, extracts IP from the configured header and returns an error
/// if the header is missing or invalid (does NOT fall back to SocketAddr).
///
/// If `ip_extractor` is None, uses the SocketAddr from ConnectInfo.
pub fn extract_client_ip<T: HasHeadersAndExtensions>(
    source: &T,
    ip_extractor: Option<&IpExtractor>,
) -> Result<String, &'static str> {
    match ip_extractor {
        Some(extractor) => {
            #[cfg(feature = "test-mode")]
            // Empty header name means use the parse function directly (for test-mode Local)
            if extractor.header_name.is_empty() {
                return extractor.extract("");
            }
            let header_value = source
                .headers()
                .get(extractor.header_name)
                .ok_or("IP header not present")?
                .to_str()
                .map_err(|_| "IP header contains invalid characters")?;
            extractor.extract(header_value)
        }
        None => source
            .extensions()
            .get::<ConnectInfo<SocketAddr>>()
            .map(|ci| ci.0.ip().to_string())
            .ok_or("No client IP available"),
    }
}
