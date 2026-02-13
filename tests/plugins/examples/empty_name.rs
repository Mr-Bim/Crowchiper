wit_bindgen::generate!({
    world: "plugin",
    path: "../../wit/plugin.wit",
});

struct EmptyNamePlugin;

impl Guest for EmptyNamePlugin {
    fn config() -> PluginConfig {
        PluginConfig {
            name: String::new(),
            version: "0.1.0".to_string(),
            hooks: vec![],
        }
    }
}

export!(EmptyNamePlugin);
