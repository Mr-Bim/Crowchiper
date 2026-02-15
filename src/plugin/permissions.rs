use std::fmt;
use std::path::PathBuf;
use std::time::Duration;

/// A single permission that can be granted to a plugin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PluginPermission {
    /// Read-only access to a host directory.
    FsRead(PathBuf),
    /// Read+write access to a host directory.
    FsWrite(PathBuf),
    /// TCP and UDP network access.
    Net,
    /// Access to a specific host environment variable.
    Env(String),
}

impl fmt::Display for PluginPermission {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PluginPermission::FsRead(p) => write!(f, "fs-read={}", p.display()),
            PluginPermission::FsWrite(p) => write!(f, "fs-write={}", p.display()),
            PluginPermission::Net => write!(f, "net"),
            PluginPermission::Env(var) => write!(f, "env-{var}"),
        }
    }
}

/// Default wall-clock timeout for plugin hook calls.
pub const DEFAULT_HOOK_TIMEOUT: Duration = Duration::from_secs(5);

/// Minimum allowed hook timeout (10ms).
const MIN_HOOK_TIMEOUT: Duration = Duration::from_millis(10);

/// A plugin path bundled with its granted permissions and config variables.
#[derive(Debug, Clone)]
pub struct PluginSpec {
    pub path: PathBuf,
    pub permissions: Vec<PluginPermission>,
    /// Key-value config pairs passed to the plugin's `config()` function.
    /// Parsed from `var-key=value` entries in the CLI spec.
    pub config: Vec<(String, String)>,
    /// Wall-clock timeout for `config()` and `on-hook()` calls.
    /// Parsed from `timeout=<N>` (seconds) or `timeout=<N>ms` (milliseconds).
    /// Defaults to 5s. Minimum 10ms.
    pub hook_timeout: Duration,
}

impl fmt::Display for PluginSpec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.path.display())?;
        let has_timeout = self.hook_timeout != DEFAULT_HOOK_TIMEOUT;
        if !self.permissions.is_empty() || !self.config.is_empty() || has_timeout {
            write!(f, ":")?;
            let mut need_comma = false;
            for perm in &self.permissions {
                if need_comma {
                    write!(f, ",")?;
                }
                write!(f, "{perm}")?;
                need_comma = true;
            }
            for (key, value) in &self.config {
                if need_comma {
                    write!(f, ",")?;
                }
                write!(f, "var-{key}={value}")?;
                need_comma = true;
            }
            if has_timeout {
                if need_comma {
                    write!(f, ",")?;
                }
                let millis = self.hook_timeout.as_millis();
                if millis % 1000 == 0 {
                    write!(f, "timeout={}", millis / 1000)?;
                } else {
                    write!(f, "timeout={millis}ms")?;
                }
            }
        }
        Ok(())
    }
}

/// Parse a `--plugin` value like `"path.wasm:net,env,fs-read=/data"` into a [`PluginSpec`].
///
/// The colon separator is only recognized when followed by a known permission prefix,
/// so paths containing colons (e.g. Windows drive letters) are handled correctly.
pub fn parse_plugin_spec(value: &str) -> Result<PluginSpec, String> {
    let (path_str, perms_str) = split_path_and_perms(value);

    if path_str.is_empty() {
        return Err("plugin path is empty".to_string());
    }

    let mut permissions = Vec::new();
    let mut config = Vec::new();
    let mut hook_timeout = DEFAULT_HOOK_TIMEOUT;
    if let Some(perms) = perms_str {
        for entry in perms.split(',') {
            let entry = entry.trim();
            if entry.is_empty() {
                continue;
            }
            if let Some(var) = entry.strip_prefix("var-") {
                let (key, value) = var.split_once('=').ok_or_else(|| {
                    format!("config variable '{entry}' must have a value (e.g., var-key=value)")
                })?;
                if key.is_empty() {
                    return Err(format!(
                        "config variable '{entry}' has an empty key (e.g., var-key=value)"
                    ));
                }
                config.push((key.to_string(), value.to_string()));
            } else if let Some(timeout_val) = entry.strip_prefix("timeout=") {
                hook_timeout = parse_timeout(timeout_val)?;
            } else {
                permissions.push(parse_single_permission(entry)?);
            }
        }
    }

    Ok(PluginSpec {
        path: PathBuf::from(path_str),
        permissions,
        config,
        hook_timeout,
    })
}

/// Validate a filesystem path used in plugin permissions.
///
/// Rejects empty paths and relative paths (must be absolute) to prevent
/// path traversal attacks.
fn validate_fs_path(path: &str, perm_name: &str) -> Result<(), String> {
    if path.is_empty() {
        return Err(format!(
            "{perm_name} requires a path (e.g., {perm_name}=/data)"
        ));
    }
    let p = std::path::Path::new(path);
    if !p.is_absolute() {
        return Err(format!(
            "{perm_name} requires an absolute path, got relative path '{path}'"
        ));
    }
    Ok(())
}

/// Parse a timeout value like `"5"` (seconds) or `"500ms"` (milliseconds).
///
/// Returns an error if the value is not a valid number or below the minimum (10ms).
fn parse_timeout(value: &str) -> Result<Duration, String> {
    let duration = if let Some(ms_str) = value.strip_suffix("ms") {
        let ms: u64 = ms_str.parse().map_err(|_| {
            format!(
                "invalid timeout '{value}': expected a number (e.g., timeout=5 or timeout=500ms)"
            )
        })?;
        Duration::from_millis(ms)
    } else {
        let secs: u64 = value.parse().map_err(|_| {
            format!(
                "invalid timeout '{value}': expected a number (e.g., timeout=5 or timeout=500ms)"
            )
        })?;
        Duration::from_secs(secs)
    };
    if duration < MIN_HOOK_TIMEOUT {
        return Err(format!(
            "timeout must be at least {}ms, got {}ms",
            MIN_HOOK_TIMEOUT.as_millis(),
            duration.as_millis()
        ));
    }
    Ok(duration)
}

fn parse_single_permission(s: &str) -> Result<PluginPermission, String> {
    match s {
        "net" => Ok(PluginPermission::Net),
        _ if s.starts_with("env-") => {
            let var_name = &s["env-".len()..];
            if var_name.is_empty() {
                return Err("env- requires a variable name (e.g., env-HOME)".to_string());
            }
            Ok(PluginPermission::Env(var_name.to_string()))
        }
        _ if s.starts_with("fs-read=") => {
            let path = &s["fs-read=".len()..];
            validate_fs_path(path, "fs-read")?;
            Ok(PluginPermission::FsRead(PathBuf::from(path)))
        }
        _ if s.starts_with("fs-write=") => {
            let path = &s["fs-write=".len()..];
            validate_fs_path(path, "fs-write")?;
            Ok(PluginPermission::FsWrite(PathBuf::from(path)))
        }
        _ => Err(format!(
            "unknown permission '{s}'. Valid: net, env-<VAR>, fs-read=<path>, fs-write=<path>, var-<key>=<value>, timeout=<secs|ms>"
        )),
    }
}

/// Split `"path.wasm:net,env"` into `("path.wasm", Some("net,env"))`.
///
/// Splits on the first `:` unless what follows looks like a Windows path
/// (single char before the colon, followed by `\` or `/`).
fn split_path_and_perms(value: &str) -> (&str, Option<&str>) {
    for (i, _) in value.match_indices(':') {
        let after = &value[i + 1..];
        // Skip Windows drive letters like C:\ or C:/
        if i == 1 && (after.starts_with('\\') || after.starts_with('/')) {
            continue;
        }
        if !after.is_empty() {
            return (&value[..i], Some(after));
        }
    }

    (value, None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_no_permissions() {
        let spec = parse_plugin_spec("a.wasm").unwrap();
        assert_eq!(spec.path, PathBuf::from("a.wasm"));
        assert!(spec.permissions.is_empty());
        assert!(spec.config.is_empty());
    }

    #[test]
    fn parse_net() {
        let spec = parse_plugin_spec("a.wasm:net").unwrap();
        assert_eq!(spec.permissions, vec![PluginPermission::Net]);
    }

    #[test]
    fn parse_env() {
        let spec = parse_plugin_spec("a.wasm:env-HOME").unwrap();
        assert_eq!(
            spec.permissions,
            vec![PluginPermission::Env("HOME".to_string())]
        );
    }

    #[test]
    fn parse_bare_env_rejected() {
        let err = parse_plugin_spec("a.wasm:env").unwrap_err();
        assert!(err.contains("unknown permission"), "got: {err}");
    }

    #[test]
    fn parse_env_empty_var_rejected() {
        let err = parse_plugin_spec("a.wasm:env-").unwrap_err();
        assert!(err.contains("requires a variable name"), "got: {err}");
    }

    #[test]
    fn parse_multiple_env_vars() {
        let spec = parse_plugin_spec("a.wasm:env-HOME,env-PATH").unwrap();
        assert_eq!(spec.permissions.len(), 2);
        assert!(
            spec.permissions
                .contains(&PluginPermission::Env("HOME".to_string()))
        );
        assert!(
            spec.permissions
                .contains(&PluginPermission::Env("PATH".to_string()))
        );
    }

    #[test]
    fn parse_multiple() {
        let spec = parse_plugin_spec("a.wasm:net,env-PATH,fs-read=/data").unwrap();
        assert_eq!(spec.permissions.len(), 3);
        assert!(spec.permissions.contains(&PluginPermission::Net));
        assert!(
            spec.permissions
                .contains(&PluginPermission::Env("PATH".to_string()))
        );
        assert!(
            spec.permissions
                .contains(&PluginPermission::FsRead(PathBuf::from("/data")))
        );
    }

    #[test]
    fn parse_fs_write() {
        let spec = parse_plugin_spec("path/to/plugin.wasm:fs-write=/tmp/data").unwrap();
        assert_eq!(
            spec.permissions,
            vec![PluginPermission::FsWrite(PathBuf::from("/tmp/data"))]
        );
    }

    #[test]
    fn parse_unknown_permission() {
        let err = parse_plugin_spec("a.wasm:bogus").unwrap_err();
        assert!(err.contains("unknown permission"), "got: {err}");
        assert!(err.contains("bogus"), "got: {err}");
    }

    #[test]
    fn parse_empty_fs_read_path() {
        let err = parse_plugin_spec("a.wasm:fs-read=").unwrap_err();
        assert!(err.contains("requires a path"), "got: {err}");
    }

    #[test]
    fn parse_empty_fs_write_path() {
        let err = parse_plugin_spec("a.wasm:fs-write=").unwrap_err();
        assert!(err.contains("requires a path"), "got: {err}");
    }

    #[test]
    fn parse_empty_path() {
        let err = parse_plugin_spec(":net").unwrap_err();
        assert!(err.contains("plugin path is empty"), "got: {err}");
    }

    #[test]
    fn windows_drive_letter_is_part_of_path() {
        let spec = parse_plugin_spec("C:\\plugins\\a.wasm").unwrap();
        assert_eq!(spec.path, PathBuf::from("C:\\plugins\\a.wasm"));
        assert!(spec.permissions.is_empty());
    }

    #[test]
    fn windows_path_with_permissions() {
        let spec = parse_plugin_spec("C:\\plugins\\a.wasm:net").unwrap();
        assert_eq!(spec.path, PathBuf::from("C:\\plugins\\a.wasm"));
        assert_eq!(spec.permissions, vec![PluginPermission::Net]);
    }

    #[test]
    fn display_roundtrip() {
        let spec = parse_plugin_spec("a.wasm:net,env-HOME,fs-read=/data").unwrap();
        let displayed = spec.to_string();
        assert!(displayed.contains("a.wasm"), "got: {displayed}");
        assert!(displayed.contains("net"), "got: {displayed}");
        assert!(displayed.contains("env-HOME"), "got: {displayed}");
        assert!(displayed.contains("fs-read=/data"), "got: {displayed}");
    }

    #[test]
    fn parse_var_config() {
        let spec = parse_plugin_spec("a.wasm:var-path=/tmp/test").unwrap();
        assert!(spec.permissions.is_empty());
        assert_eq!(
            spec.config,
            vec![("path".to_string(), "/tmp/test".to_string())]
        );
    }

    #[test]
    fn parse_var_mixed_with_permissions() {
        let spec = parse_plugin_spec("a.wasm:net,var-path=/data,env-HOME").unwrap();
        assert_eq!(
            spec.permissions,
            vec![
                PluginPermission::Net,
                PluginPermission::Env("HOME".to_string())
            ]
        );
        assert_eq!(spec.config, vec![("path".to_string(), "/data".to_string())]);
    }

    #[test]
    fn parse_multiple_vars() {
        let spec = parse_plugin_spec("a.wasm:var-key1=val1,var-key2=val2").unwrap();
        assert_eq!(spec.config.len(), 2);
        assert_eq!(spec.config[0], ("key1".to_string(), "val1".to_string()));
        assert_eq!(spec.config[1], ("key2".to_string(), "val2".to_string()));
    }

    #[test]
    fn parse_var_empty_value_is_ok() {
        let spec = parse_plugin_spec("a.wasm:var-key=").unwrap();
        assert_eq!(spec.config, vec![("key".to_string(), String::new())]);
    }

    #[test]
    fn parse_var_missing_equals() {
        let err = parse_plugin_spec("a.wasm:var-key").unwrap_err();
        assert!(err.contains("must have a value"), "got: {err}");
    }

    #[test]
    fn parse_var_empty_key() {
        let err = parse_plugin_spec("a.wasm:var-=value").unwrap_err();
        assert!(err.contains("empty key"), "got: {err}");
    }

    #[test]
    fn display_with_vars() {
        let spec = parse_plugin_spec("a.wasm:net,var-path=/tmp").unwrap();
        let displayed = spec.to_string();
        assert!(displayed.contains("net"), "got: {displayed}");
        assert!(displayed.contains("var-path=/tmp"), "got: {displayed}");
    }

    // ── Timeout ──────────────────────────────────────────────────────

    #[test]
    fn parse_default_timeout() {
        let spec = parse_plugin_spec("a.wasm").unwrap();
        assert_eq!(spec.hook_timeout, Duration::from_secs(5));
    }

    #[test]
    fn parse_timeout_seconds() {
        let spec = parse_plugin_spec("a.wasm:timeout=10").unwrap();
        assert_eq!(spec.hook_timeout, Duration::from_secs(10));
    }

    #[test]
    fn parse_timeout_milliseconds() {
        let spec = parse_plugin_spec("a.wasm:timeout=500ms").unwrap();
        assert_eq!(spec.hook_timeout, Duration::from_millis(500));
    }

    #[test]
    fn parse_timeout_minimum_10ms() {
        let spec = parse_plugin_spec("a.wasm:timeout=10ms").unwrap();
        assert_eq!(spec.hook_timeout, Duration::from_millis(10));
    }

    #[test]
    fn parse_timeout_below_minimum_rejected() {
        let err = parse_plugin_spec("a.wasm:timeout=5ms").unwrap_err();
        assert!(err.contains("at least 10ms"), "got: {err}");
    }

    #[test]
    fn parse_timeout_zero_rejected() {
        let err = parse_plugin_spec("a.wasm:timeout=0").unwrap_err();
        assert!(err.contains("at least 10ms"), "got: {err}");
    }

    #[test]
    fn parse_timeout_invalid_number() {
        let err = parse_plugin_spec("a.wasm:timeout=abc").unwrap_err();
        assert!(err.contains("invalid timeout"), "got: {err}");
    }

    #[test]
    fn parse_timeout_mixed_with_permissions() {
        let spec = parse_plugin_spec("a.wasm:net,timeout=2,env-HOME").unwrap();
        assert_eq!(spec.hook_timeout, Duration::from_secs(2));
        assert_eq!(spec.permissions.len(), 2);
    }

    #[test]
    fn display_with_non_default_timeout_seconds() {
        let spec = parse_plugin_spec("a.wasm:net,timeout=10").unwrap();
        let displayed = spec.to_string();
        assert!(displayed.contains("timeout=10"), "got: {displayed}");
        // Should not have "ms" suffix for whole seconds
        assert!(!displayed.contains("ms"), "got: {displayed}");
    }

    #[test]
    fn display_with_non_default_timeout_ms() {
        let spec = parse_plugin_spec("a.wasm:timeout=500ms").unwrap();
        let displayed = spec.to_string();
        assert!(displayed.contains("timeout=500ms"), "got: {displayed}");
    }

    #[test]
    fn display_omits_default_timeout() {
        let spec = parse_plugin_spec("a.wasm:net").unwrap();
        let displayed = spec.to_string();
        assert!(!displayed.contains("timeout"), "got: {displayed}");
    }
}
