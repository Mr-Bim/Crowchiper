wit_bindgen::generate!({
    world: "plugin",
    path: "../../wit/plugin.wit",
});

struct InfiniteLoopPlugin;

impl Guest for InfiniteLoopPlugin {
    fn config(_config: Vec<(String, String)>) -> PluginConfig {
        loop {}
    }

    fn on_hook(_event: HookEvent) -> Result<(), String> {
        Ok(())
    }
}

export!(InfiniteLoopPlugin);
