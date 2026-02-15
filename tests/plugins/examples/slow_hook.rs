wit_bindgen::generate!({
    world: "plugin",
    path: "../../wit/plugin.wit",
});

struct SlowHookPlugin;

impl Guest for SlowHookPlugin {
    fn config(_config: Vec<(String, String)>) -> PluginConfig {
        PluginConfig {
            name: "slow-hook".to_string(),
            version: "1.0.0".to_string(),
            target: HookTarget::Server,
            hooks: vec![Hook::Server(ServerHook::IpChange)],
        }
    }

    fn on_hook(_event: HookEvent) -> Result<(), String> {
        // Sleep for 60 seconds. In WASI this becomes a host clock/poll call,
        // consuming zero fuel. The wall-clock timeout should terminate this.
        std::thread::sleep(std::time::Duration::from_secs(60));
        Ok(())
    }
}

export!(SlowHookPlugin);
