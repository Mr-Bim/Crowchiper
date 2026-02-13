wit_bindgen::generate!({
    world: "plugin",
    path: "../../wit/plugin.wit",
});

struct HookEchoPlugin;

impl Guest for HookEchoPlugin {
    fn config(_config: Vec<(String, String)>) -> PluginConfig {
        PluginConfig {
            name: "hook-echo".to_string(),
            version: "1.0.0".to_string(),
            target: HookTarget::Server,
            hooks: vec![Hook::Server(ServerHook::IpChange)],
        }
    }

    fn on_hook(event: HookEvent) -> Result<(), String> {
        eprintln!("hook={:?} time={}", event.hook, event.time);
        for (key, value) in &event.values {
            eprintln!("  {key}={value}");
        }
        Ok(())
    }
}

export!(HookEchoPlugin);
