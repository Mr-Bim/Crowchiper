wit_bindgen::generate!({
    world: "plugin",
    path: "../../wit/plugin.wit",
});

struct GoodPlugin;

impl Guest for GoodPlugin {
    fn config(_config: Vec<(String, String)>) -> PluginConfig {
        PluginConfig {
            name: "good".to_string(),
            version: "1.0.0".to_string(),
            target: HookTarget::Server,
            hooks: vec![Hook::Server(ServerHook::IpChange)],
        }
    }

    fn on_hook(_event: HookEvent) -> Result<(), String> {
        Ok(())
    }
}

export!(GoodPlugin);
