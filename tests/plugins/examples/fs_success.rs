wit_bindgen::generate!({
    world: "plugin",
    path: "../../wit/plugin.wit",
});

struct FsSuccessPlugin;

impl Guest for FsSuccessPlugin {
    fn config(config: Vec<(String, String)>) -> PluginConfig {
        let path = config
            .iter()
            .find(|(k, _)| k == "path")
            .map(|(_, v)| v.as_str())
            .expect("fs-success plugin requires a 'path' config variable");
        let test_content = "hello from plugin";
        let file = format!("{path}/output.txt");
        std::fs::write(&file, test_content).unwrap();
        let read_back = std::fs::read_to_string(&file).unwrap();
        assert_eq!(read_back, test_content);
        PluginConfig {
            name: "fs-success".to_string(),
            version: "1.0.0".to_string(),
            hooks: vec![],
        }
    }
}

export!(FsSuccessPlugin);
