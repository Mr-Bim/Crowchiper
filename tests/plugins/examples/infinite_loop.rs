wit_bindgen::generate!({
    world: "plugin",
    path: "../../wit/plugin.wit",
});

struct InfiniteLoopPlugin;

impl Guest for InfiniteLoopPlugin {
    fn config(_config: Vec<(String, String)>) -> PluginConfig {
        loop {}
    }
}

export!(InfiniteLoopPlugin);
