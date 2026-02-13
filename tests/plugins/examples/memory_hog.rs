wit_bindgen::generate!({
    world: "plugin",
    path: "../../wit/plugin.wit",
});

struct MemoryHogPlugin;

impl Guest for MemoryHogPlugin {
    fn config(_config: Vec<(String, String)>) -> PluginConfig {
        let mut vecs = Vec::new();
        loop {
            vecs.push(vec![0u8; 1_000_000]);
        }
    }
}

export!(MemoryHogPlugin);
