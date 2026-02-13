use std::fmt;

#[derive(Debug)]
pub enum PluginError {
    Load(String),
    Runtime(String),
    InvalidConfig(String),
}

impl fmt::Display for PluginError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PluginError::Load(msg) => write!(f, "plugin load error: {msg}"),
            PluginError::Runtime(msg) => write!(f, "plugin runtime error: {msg}"),
            PluginError::InvalidConfig(msg) => write!(f, "plugin config error: {msg}"),
        }
    }
}

impl std::error::Error for PluginError {}
