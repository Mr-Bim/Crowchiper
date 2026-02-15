use std::path::Path;

use wasmtime_wasi::WasiCtxBuilder;

use super::permissions::PluginPermission;
use super::{Hook, HookTarget, PluginError};

/// Derive the target from a hook variant.
pub(crate) fn hook_target(hook: &Hook) -> HookTarget {
    match hook {
        Hook::Server(_) => HookTarget::Server,
    }
}

/// Apply granted permissions to the WASI context builder.
///
/// Each permission maps to a specific `WasiCtxBuilder` method:
/// - `FsRead` → `preopened_dir` with read-only perms
/// - `FsWrite` → `preopened_dir` with read+write perms
/// - `Net` → `inherit_network`
/// - `Env` → `inherit_env`
pub(crate) fn apply_permissions(
    builder: &mut WasiCtxBuilder,
    permissions: &[PluginPermission],
) -> Result<(), PluginError> {
    use wasmtime_wasi::{DirPerms, FilePerms};

    for perm in permissions {
        match perm {
            PluginPermission::FsRead(host_path) => {
                let canonical = canonicalize_plugin_path(host_path)?;
                let guest_path = canonical.to_str().ok_or_else(|| {
                    PluginError::Load(format!(
                        "filesystem path is not valid UTF-8: {}",
                        canonical.display()
                    ))
                })?;
                builder
                    .preopened_dir(&canonical, guest_path, DirPerms::READ, FilePerms::READ)
                    .map_err(|e| {
                        PluginError::Load(format!(
                            "failed to preopen directory '{}': {e}",
                            canonical.display()
                        ))
                    })?;
            }
            PluginPermission::FsWrite(host_path) => {
                let canonical = canonicalize_plugin_path(host_path)?;
                let guest_path = canonical.to_str().ok_or_else(|| {
                    PluginError::Load(format!(
                        "filesystem path is not valid UTF-8: {}",
                        canonical.display()
                    ))
                })?;
                builder
                    .preopened_dir(
                        &canonical,
                        guest_path,
                        DirPerms::READ | DirPerms::MUTATE,
                        FilePerms::READ | FilePerms::WRITE,
                    )
                    .map_err(|e| {
                        PluginError::Load(format!(
                            "failed to preopen directory '{}': {e}",
                            canonical.display()
                        ))
                    })?;
            }
            PluginPermission::Net => {
                builder.inherit_network();
            }
            PluginPermission::Env(var_name) => {
                if let Ok(value) = std::env::var(var_name) {
                    builder.env(var_name, &value);
                }
            }
        }
    }
    Ok(())
}

/// Canonicalize a filesystem path for plugin preopening.
///
/// Resolves symlinks and `..` components so the WASI sandbox operates on the
/// real path. This prevents a plugin from escaping its sandbox via symlinks
/// or path traversal in the preopened directory.
fn canonicalize_plugin_path(path: &Path) -> Result<std::path::PathBuf, PluginError> {
    std::fs::canonicalize(path).map_err(|e| {
        PluginError::Load(format!(
            "failed to resolve filesystem path '{}': {e}",
            path.display()
        ))
    })
}

/// Extract structured panic info from WASI stderr output.
///
/// Rust panics in WASM write to stderr in the format:
/// `thread '...' panicked at <location>:\n<message>\nnote: ...`
pub(crate) fn extract_panic_message(stderr: &str) -> Option<String> {
    let after_marker = stderr.split("panicked at ").nth(1)?;
    let (location, rest) = after_marker.split_once('\n')?;
    let location = location.trim_end_matches(':');

    // Get the panic message, trimming the trailing "note: ..." line
    let message = rest
        .split_once("\nnote:")
        .map_or(rest, |(msg, _)| msg)
        .trim();

    // Try to extract the inner error from unwrap() output:
    // `called \`Result::unwrap()\` on an \`Err\` value: <debug repr>`
    let detail = message
        .split_once("` value: ")
        .map_or(message, |(_, inner)| inner);

    // Try to extract quoted error string from Debug repr like:
    // `Custom { kind: Uncategorized, error: "actual message" }`
    let detail = extract_quoted_error(detail).unwrap_or(detail);

    let detail = sanitize_plugin_output(detail);
    let location = sanitize_plugin_output(location);

    Some(format!("panicked at {location}: {detail}"))
}

/// Sanitize plugin output to prevent log injection.
///
/// - Replaces `\n` and `\r` with their escaped forms (keeps output on one log line)
/// - Strips ANSI escape sequences (CSI `\x1b[...` and OSC `\x1b]...`)
/// - Removes other ASCII control characters (0x00–0x1F except `\t`, and 0x7F)
pub(crate) fn sanitize_plugin_output(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut iter = s.chars().peekable();
    while let Some(c) = iter.next() {
        match c {
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\x1b' => {
                // Strip ANSI escape sequences: CSI (ESC [) and OSC (ESC ])
                match iter.peek() {
                    Some('[') | Some(']') => {
                        let is_osc = *iter.peek().unwrap() == ']';
                        iter.next(); // consume '[' or ']'
                        for c in iter.by_ref() {
                            if is_osc {
                                // OSC ends with BEL (\x07) or ST (ESC \)
                                if c == '\x07' {
                                    break;
                                }
                                if c == '\x1b' {
                                    if iter.peek() == Some(&'\\') {
                                        iter.next();
                                    }
                                    break;
                                }
                            } else {
                                // CSI ends at first byte in 0x40–0x7E
                                if ('@'..='~').contains(&c) {
                                    break;
                                }
                            }
                        }
                    }
                    // Bare ESC followed by something else — drop the ESC
                    _ => {}
                }
            }
            '\t' => out.push('\t'),
            // Drop other control characters (0x00–0x1F, 0x7F)
            c if c.is_ascii_control() => {}
            c => out.push(c),
        }
    }
    out
}

/// Extract the inner `error: "..."` value from a Debug-formatted std::io::Error.
fn extract_quoted_error(detail: &str) -> Option<&str> {
    let after = detail.split("error: \"").nth(1)?;
    // Find the closing quote (handle escaped quotes)
    let mut chars = after.char_indices();
    while let Some((i, c)) = chars.next() {
        match c {
            '\\' => {
                chars.next(); // skip escaped char
            }
            '"' => return Some(&after[..i]),
            _ => {}
        }
    }
    None
}
