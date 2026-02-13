wit_bindgen::generate!({
    world: "plugin",
    path: "../../wit/plugin.wit",
});

struct FsErrorPlugin;

impl Guest for FsErrorPlugin {
    fn config(_config: Vec<(String, String)>) -> PluginConfig {
        std::fs::write("a.txt", "hello world").unwrap();
        PluginConfig {
            name: "fs-error".to_string(),
            version: "0.1.0".to_string(),
            hooks: vec![],
        }
    }
}

export!(FsErrorPlugin);
