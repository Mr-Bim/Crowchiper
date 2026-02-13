mod error;
mod permissions;
mod runtime;

pub use error::PluginError;
pub use permissions::{PluginPermission, PluginSpec, parse_plugin_spec};
pub use runtime::PluginRuntime;

wasmtime::component::bindgen!({
    world: "plugin",
    path: "wit/plugin.wit",
});
