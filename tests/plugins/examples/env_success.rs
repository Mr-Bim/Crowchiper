wit_bindgen::generate!({
    world: "plugin",
    path: "../../wit/plugin.wit",
});

struct EnvSuccessPlugin;

impl Guest for EnvSuccessPlugin {
    fn config(_config: Vec<(String, String)>) -> PluginConfig {
        let val = std::env::var("TEST_PLUGIN_VAR").unwrap();
        PluginConfig {
            name: format!("env-success-{val}"),
            version: "1.0.0".to_string(),
            target: HookTarget::Server,
            hooks: vec![],
        }
    }

    fn on_hook(_event: HookEvent) -> Result<(), String> {
        Ok(())
    }
}

export!(EnvSuccessPlugin);
