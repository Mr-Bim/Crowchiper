use wasmtime::StoreLimits;
use wasmtime_wasi::{ResourceTable, WasiCtx, WasiCtxView, WasiView};

use super::helpers::sanitize_plugin_output;
use super::{LogLevel, PluginImports};

/// Maximum bytes of log output per single `log()` call from a plugin.
const LOG_MESSAGE_LIMIT: usize = 4096;

/// Per-instance WASI state for a plugin.
///
/// Each plugin gets its own sandboxed WASI context (filesystem, stdio, env),
/// resource table (handles for host resources like file descriptors), and
/// resource limits (memory, tables) to prevent unbounded allocation.
/// The plugin cannot access anything outside what is explicitly granted here.
pub(crate) struct PluginState {
    pub(crate) wasi: WasiCtx,
    pub(crate) table: ResourceTable,
    pub(crate) limits: StoreLimits,
    /// Plugin name used for log output attribution (set from file path stem).
    pub(crate) plugin_name: String,
}

/// Implements the `WasiView` trait so wasmtime can access the WASI context
/// and resource table from our custom store data.
impl WasiView for PluginState {
    fn ctx(&mut self) -> WasiCtxView<'_> {
        WasiCtxView {
            ctx: &mut self.wasi,
            table: &mut self.table,
        }
    }
}

/// Implements the host-provided `log` import so plugins can log messages
/// directly to the server's tracing infrastructure.
impl PluginImports for PluginState {
    fn log(&mut self, level: LogLevel, msg: String) {
        let clean = sanitize_plugin_output(&msg);
        if clean.is_empty() {
            return;
        }
        let clean = if clean.len() > LOG_MESSAGE_LIMIT {
            &clean[..LOG_MESSAGE_LIMIT]
        } else {
            &clean
        };
        match level {
            LogLevel::Debug => tracing::debug!(plugin = %self.plugin_name, "{clean}"),
            LogLevel::Info => tracing::info!(plugin = %self.plugin_name, "{clean}"),
            LogLevel::Warn => tracing::warn!(plugin = %self.plugin_name, "{clean}"),
            LogLevel::Error => tracing::error!(plugin = %self.plugin_name, "{clean}"),
        }
    }
}
