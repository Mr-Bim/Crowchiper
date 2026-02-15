wit_bindgen::generate!({
    world: "plugin",
    path: "../../wit/plugin.wit",
});

struct LogTestPlugin;

impl Guest for LogTestPlugin {
    fn config(_config: Vec<(String, String)>) -> PluginConfig {
        log(LogLevel::Info, "config called");
        PluginConfig {
            name: "log-test".to_string(),
            version: "1.0.0".to_string(),
            target: HookTarget::Server,
            hooks: vec![Hook::Server(ServerHook::IpChange)],
        }
    }

    fn on_hook(_event: HookEvent) -> Result<(), String> {
        log(LogLevel::Debug, "debug message");
        log(LogLevel::Info, "info message");
        log(LogLevel::Warn, "warn message");
        log(LogLevel::Error, "error message");
        Ok(())
    }
}

export!(LogTestPlugin);
