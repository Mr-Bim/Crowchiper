wit_bindgen::generate!({
    world: "plugin",
    path: "../../wit/plugin.wit",
});

struct EnvErrorPlugin;

impl Guest for EnvErrorPlugin {
    fn config() -> PluginConfig {
        std::env::var("SECRET").unwrap();
        PluginConfig {
            name: "env-error".to_string(),
            version: "0.1.0".to_string(),
            hooks: vec![],
        }
    }
}

export!(EnvErrorPlugin);
