wit_bindgen::generate!({
    world: "plugin",
    path: "../../wit/plugin.wit",
});

struct NetErrorPlugin;

impl Guest for NetErrorPlugin {
    fn config() -> PluginConfig {
        std::net::TcpStream::connect("127.0.0.1:80").unwrap();
        PluginConfig {
            name: "net-error".to_string(),
            version: "0.1.0".to_string(),
            hooks: vec![],
        }
    }
}

export!(NetErrorPlugin);
