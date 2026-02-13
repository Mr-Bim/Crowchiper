wit_bindgen::generate!({
    world: "plugin",
    path: "../../wit/plugin.wit",
});

struct GoodPlugin;

impl Guest for GoodPlugin {
    fn config() -> PluginConfig {
        PluginConfig {
            name: "good".to_string(),
            version: "1.0.0".to_string(),
            hooks: vec!["test-hook".to_string()],
        }
    }
}

export!(GoodPlugin);
