//! Global server configuration initialized once at startup.
//!
//! These values are immutable after initialization and shared across all request handlers.
//! In test mode, values can be overwritten to support different test configurations.

use std::sync::Arc;

use crate::cli::IpExtractor;
use crate::plugin::PluginManager;

// In test mode we need RwLock so different tests can use different configs.
// In production we use OnceLock for zero-overhead reads.

#[cfg(not(feature = "test-mode"))]
mod inner {
    use std::sync::OnceLock;

    use super::*;

    static SECURE_COOKIES: OnceLock<bool> = OnceLock::new();
    static IP_EXTRACTOR: OnceLock<Option<IpExtractor>> = OnceLock::new();
    static PLUGIN_MANAGER: OnceLock<Option<Arc<PluginManager>>> = OnceLock::new();

    pub fn init(
        secure_cookies: bool,
        ip_extractor: Option<IpExtractor>,
        plugin_manager: Option<Arc<PluginManager>>,
    ) {
        SECURE_COOKIES.get_or_init(|| secure_cookies);
        IP_EXTRACTOR.get_or_init(|| ip_extractor);
        PLUGIN_MANAGER.get_or_init(|| plugin_manager);
    }

    pub fn secure_cookies() -> bool {
        *SECURE_COOKIES.get().expect("server config not initialized")
    }

    pub fn ip_extractor() -> Option<IpExtractor> {
        IP_EXTRACTOR
            .get()
            .expect("server config not initialized")
            .clone()
    }

    pub fn plugin_manager() -> Option<Arc<PluginManager>> {
        PLUGIN_MANAGER
            .get()
            .expect("server config not initialized")
            .clone()
    }
}

#[cfg(feature = "test-mode")]
mod inner {
    use std::sync::RwLock;

    use super::*;

    static SECURE_COOKIES: RwLock<Option<bool>> = RwLock::new(None);
    static IP_EXTRACTOR: RwLock<Option<Option<IpExtractor>>> = RwLock::new(None);
    static PLUGIN_MANAGER: RwLock<Option<Option<Arc<PluginManager>>>> = RwLock::new(None);

    pub fn init(
        secure_cookies: bool,
        ip_extractor: Option<IpExtractor>,
        plugin_manager: Option<Arc<PluginManager>>,
    ) {
        *SECURE_COOKIES.write().unwrap() = Some(secure_cookies);
        *IP_EXTRACTOR.write().unwrap() = Some(ip_extractor);
        *PLUGIN_MANAGER.write().unwrap() = Some(plugin_manager);
    }

    pub fn secure_cookies() -> bool {
        SECURE_COOKIES
            .read()
            .unwrap()
            .expect("server config not initialized")
    }

    pub fn ip_extractor() -> Option<IpExtractor> {
        IP_EXTRACTOR
            .read()
            .unwrap()
            .as_ref()
            .expect("server config not initialized")
            .clone()
    }

    pub fn plugin_manager() -> Option<Arc<PluginManager>> {
        PLUGIN_MANAGER
            .read()
            .unwrap()
            .as_ref()
            .expect("server config not initialized")
            .clone()
    }
}

pub use inner::*;
