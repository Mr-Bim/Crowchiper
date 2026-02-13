wit_bindgen::generate!({
    world: "plugin",
    path: "../../wit/plugin.wit",
});

struct StackOverflowPlugin;

impl Guest for StackOverflowPlugin {
    fn config(_config: Vec<(String, String)>) -> PluginConfig {
        fn recurse(n: u64) -> u64 {
            recurse(n + 1) + n
        }
        let _ = recurse(0);
        unreachable!()
    }
}

export!(StackOverflowPlugin);
