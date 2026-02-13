wit_bindgen::generate!({
    world: "plugin",
    path: "../../wit/plugin.wit",
});

struct StackOverflowPlugin;

impl Guest for StackOverflowPlugin {
    fn config(_config: Vec<(String, String)>) -> PluginConfig {
        #[allow(unconditional_recursion)]
        fn recurse(n: u64) -> u64 {
            recurse(n + 1) + n
        }
        let _ = recurse(0);
        unreachable!()
    }

    fn on_hook(_event: HookEvent) -> Result<(), String> {
        Ok(())
    }
}

export!(StackOverflowPlugin);
