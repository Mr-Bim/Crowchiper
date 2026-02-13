wit_bindgen::generate!({
    world: "plugin",
    path: "../../wit/plugin.wit",
});

struct NetSuccessPlugin;

impl Guest for NetSuccessPlugin {
    fn config(_config: Vec<(String, String)>) -> PluginConfig {
        // Try connecting — connection refused is expected (port 1 is not listening).
        // "not supported" or "Permission denied" means the sandbox blocked it.
        match std::net::TcpStream::connect("127.0.0.1:1") {
            Ok(_) => {}
            Err(e) => {
                let msg = e.to_string();
                if msg.contains("not supported") || msg.contains("Permission denied") {
                    panic!("Network access blocked by sandbox: {e}");
                }
                // Connection refused is fine — proves network access works
            }
        }
        PluginConfig {
            name: "net-success".to_string(),
            version: "1.0.0".to_string(),
            target: HookTarget::Server,
            hooks: vec![],
        }
    }

    fn on_hook(_event: HookEvent) -> Result<(), String> {
        Ok(())
    }
}

export!(NetSuccessPlugin);
