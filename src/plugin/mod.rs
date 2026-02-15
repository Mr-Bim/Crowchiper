mod error;
mod helpers;
mod manager;
mod permissions;
mod runtime;
mod state;

pub use error::PluginError;
pub use manager::PluginManager;
pub use permissions::{DEFAULT_HOOK_TIMEOUT, PluginPermission, PluginSpec, parse_plugin_spec};
pub use runtime::PluginRuntime;

wasmtime::component::bindgen!({
    world: "plugin",
    path: "wit/plugin.wit",
    with: {},
    additional_derives: [Clone, PartialEq, Eq, Hash],
    require_store_data_send: true,
    exports: {
        default: async,
    },
});
