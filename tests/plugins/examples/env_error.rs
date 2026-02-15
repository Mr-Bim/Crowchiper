wit_bindgen::generate!({
    world: "plugin",
    path: "../../wit/plugin.wit",
});

struct EnvErrorPlugin;

impl Guest for EnvErrorPlugin {
    fn config(_config: Vec<(String, String)>) -> PluginConfig {
        std::env::var("SECRET").unwrap();
        PluginConfig {
            name: "env-error".to_string(),
            version: "0.1.0".to_string(),
            target: HookTarget::Server,
            hooks: vec![],
        }
    }

    fn on_hook(_event: HookEvent) -> Result<(), String> {
        Ok(())
    }
}

export!(EnvErrorPlugin);
