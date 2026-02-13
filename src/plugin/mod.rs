mod error;
mod runtime;

pub use error::PluginError;
pub use runtime::PluginRuntime;

wasmtime::component::bindgen!({
    world: "plugin",
    path: "wit/plugin.wit",
});
