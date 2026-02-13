use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use wasmtime::component::{Component, Linker};
use wasmtime::{Engine, Store, StoreLimits, StoreLimitsBuilder};
use wasmtime_wasi::p2::pipe::MemoryOutputPipe;
use wasmtime_wasi::{ResourceTable, WasiCtx, WasiCtxBuilder, WasiCtxView, WasiView};

use super::permissions::PluginPermission;
use super::{Hook, HookEvent, HookTarget, Plugin, PluginError};

/// Per-instance WASI state for a plugin.
///
/// Each plugin gets its own sandboxed WASI context (filesystem, stdio, env),
/// resource table (handles for host resources like file descriptors), and
/// resource limits (memory, tables) to prevent unbounded allocation.
/// The plugin cannot access anything outside what is explicitly granted here.
struct PluginState {
    wasi: WasiCtx,
    table: ResourceTable,
    limits: StoreLimits,
}

/// Implements the `WasiView` trait so wasmtime can access the WASI context
/// and resource table from our custom store data.
impl WasiView for PluginState {
    fn ctx(&mut self) -> WasiCtxView<'_> {
        WasiCtxView {
            ctx: &mut self.wasi,
            table: &mut self.table,
        }
    }
}

/// A loaded and validated WASM plugin.
///
/// `PluginRuntime` holds the metadata extracted from a plugin's `config()` export
/// after successfully loading, compiling, and instantiating the WASM component.
/// It does **not** keep the wasmtime engine or store alive — those are created
/// transiently during `load()` and dropped once the config is retrieved.
///
/// # Lifecycle
///
/// 1. **Read** — The `.wasm` file is read from disk.
/// 2. **Compile** — The bytes are compiled into a wasmtime `Component`.
/// 3. **Link** — WASI imports are wired up (only stderr is captured; no filesystem,
///    env, or network access is granted to the plugin).
/// 4. **Instantiate** — The component is instantiated inside a fresh `Store`.
/// 5. **Configure** — The plugin's exported `config()` function is called. This
///    returns the plugin's name, version, and the set of hooks it wants to handle.
/// 6. **Validate** — The returned config is checked (e.g. name must be non-empty).
///
/// If any step fails, a descriptive `PluginError` is returned. When `config()`
/// panics inside the WASM guest, stderr is inspected to produce a clean error
/// message (or a verbose one when `RUST_BACKTRACE` is set).
pub struct PluginRuntime {
    plugin_name: String,
    plugin_version: String,
    target: HookTarget,
    /// Hooks this plugin registered for. All must match `target`.
    hooks: Vec<Hook>,
    /// Live store and instance created at load time.
    /// Each plugin has its own Mutex so different plugins can run in parallel.
    /// A single plugin serializes its hook calls (WASM is single-threaded).
    instance: Mutex<(Store<PluginState>, Plugin)>,
    stderr: MemoryOutputPipe,
}

impl std::fmt::Debug for PluginRuntime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PluginRuntime")
            .field("plugin_name", &self.plugin_name)
            .field("plugin_version", &self.plugin_version)
            .field("target", &self.target)
            .field("hooks", &self.hooks)
            .finish_non_exhaustive()
    }
}

impl PluginRuntime {
    /// Load a WASM component plugin from disk, instantiate it, and call `config()` to validate.
    ///
    /// See the [`PluginRuntime`] struct docs for the full lifecycle.
    ///
    /// # Errors
    ///
    /// Returns [`PluginError::Load`] for I/O or compilation failures,
    /// [`PluginError::Runtime`] if instantiation or `config()` fails, and
    /// [`PluginError::InvalidConfig`] if the returned metadata is invalid.
    pub fn load(
        path: &Path,
        permissions: &[PluginPermission],
        config_vars: &[(String, String)],
    ) -> Result<Self, PluginError> {
        // Step 1: Read the raw .wasm bytes from disk.
        let wasm_bytes = std::fs::read(path)
            .map_err(|e| PluginError::Load(format!("failed to read {}: {e}", path.display())))?;

        // Step 2: Create a wasmtime engine with resource limits and compile the
        // bytes into a Component. Fuel metering caps CPU usage (each WASM instruction
        // consumes ~1 fuel unit). Stack size is capped to prevent stack overflow.
        let mut engine_config = wasmtime::Config::new();
        engine_config.consume_fuel(true);
        engine_config.max_wasm_stack(512 * 1024); // 512KB stack limit
        let engine = Engine::new(&engine_config)
            .map_err(|e| PluginError::Load(format!("failed to create engine: {e}")))?;

        let component = Component::new(&engine, &wasm_bytes)
            .map_err(|e| PluginError::Load(format!("failed to compile {}: {e}", path.display())))?;

        // Step 3: Set up the linker with WASI imports so the plugin can use
        // basic WASI functionality (currently only stderr for error reporting).
        let mut linker: Linker<PluginState> = Linker::new(&engine);
        wasmtime_wasi::p2::add_to_linker_sync(&mut linker)
            .map_err(|e| PluginError::Load(format!("failed to add WASI to linker: {e}")))?;

        // Build the WASI context. By default only stderr is captured into an
        // in-memory pipe so we can read panic messages. Additional capabilities
        // (filesystem, network, env) are granted based on the plugin's permissions.
        let stderr = MemoryOutputPipe::new(4096);
        let mut wasi_builder = WasiCtxBuilder::new();
        wasi_builder.stderr(stderr.clone());
        apply_permissions(&mut wasi_builder, permissions)?;
        let wasi = wasi_builder.build();
        let table = ResourceTable::new();

        // Memory limits: cap each linear memory at 10MB to prevent OOM.
        let limits = StoreLimitsBuilder::new()
            .memory_size(10 * 1024 * 1024)
            .build();
        let mut store = Store::new(
            &engine,
            PluginState {
                wasi,
                table,
                limits,
            },
        );
        store.limiter(|state| &mut state.limits);

        // Fuel limit: ~10M instructions for the config() call.
        store
            .set_fuel(10_000_000)
            .map_err(|e| PluginError::Load(format!("failed to set fuel limit: {e}")))?;

        // Step 4: Instantiate the component. This resolves all imports against
        // the linker and runs any WASI initialization (`_start` / `_initialize`).
        let instance = Plugin::instantiate(&mut store, &component, &linker)
            .map_err(|e| PluginError::Runtime(format!("failed to instantiate plugin: {e}")))?;

        // Step 5: Call the plugin's exported `config()` function to retrieve its
        // metadata (name, version, hooks). If the call traps (e.g. the guest
        // panics), we read stderr to produce a human-friendly error message.
        let config = instance.call_config(&mut store, config_vars).map_err(|e| {
            let stderr_bytes = stderr.contents();
            let stderr_output = String::from_utf8_lossy(&stderr_bytes);
            // In verbose mode (RUST_BACKTRACE set), include the full stderr dump.
            // Otherwise, try to extract just the panic message for a cleaner error.
            let verbose = std::env::var_os("RUST_BACKTRACE").is_some();
            let msg = if verbose {
                let mut msg = format!("failed to call config(): {e}");
                if !stderr_output.is_empty() {
                    msg = format!("{msg}\n\nplugin stderr:\n{stderr_output}");
                }
                msg
            } else {
                extract_panic_message(&stderr_output)
                    .unwrap_or_else(|| format!("failed to call config(): {e}"))
            };
            PluginError::Runtime(msg)
        })?;

        // Step 6: Validate the config — a plugin must have a non-empty name.
        if config.name.is_empty() {
            return Err(PluginError::InvalidConfig("plugin name is empty".into()));
        }

        // Validate that all hooks match the declared target.
        for hook in &config.hooks {
            let hook_target = hook_target(hook);
            if hook_target != config.target {
                return Err(PluginError::InvalidConfig(format!(
                    "hook {hook:?} has target {hook_target:?} but plugin declared target {:?}",
                    config.target
                )));
            }
        }

        // Refuel the store for future hook calls (config() consumed some fuel).
        store
            .set_fuel(10_000_000)
            .map_err(|e| PluginError::Load(format!("failed to reset fuel: {e}")))?;

        Ok(Self {
            plugin_name: config.name,
            plugin_version: config.version,
            target: config.target,
            hooks: config.hooks,
            instance: Mutex::new((store, instance)),
            stderr,
        })
    }

    pub fn name(&self) -> &str {
        &self.plugin_name
    }

    pub fn version(&self) -> &str {
        &self.plugin_version
    }

    pub fn hooks(&self) -> &[Hook] {
        &self.hooks
    }

    pub fn target(&self) -> &HookTarget {
        &self.target
    }

    /// Call the plugin's `on-hook` export with the given event.
    ///
    /// Uses the instance created at load time. Refuels the store before each
    /// call to ensure the plugin has a fresh CPU budget.
    pub fn call_hook(&self, event: &HookEvent) -> Result<(), PluginError> {
        let mut guard = self
            .instance
            .lock()
            .map_err(|_| PluginError::Hook("plugin mutex poisoned".into()))?;
        let (store, instance) = &mut *guard;

        // Refuel before each hook call.
        store
            .set_fuel(10_000_000)
            .map_err(|e| PluginError::Hook(format!("failed to set fuel limit: {e}")))?;

        let result = instance.call_on_hook(store, event).map_err(|e| {
            let stderr_bytes = self.stderr.contents();
            let stderr_output = String::from_utf8_lossy(&stderr_bytes);
            let verbose = std::env::var_os("RUST_BACKTRACE").is_some();
            let msg = if verbose {
                let mut msg = format!("failed to call on_hook(): {e}");
                if !stderr_output.is_empty() {
                    msg = format!("{msg}\n\nplugin stderr:\n{stderr_output}");
                }
                msg
            } else {
                extract_panic_message(&stderr_output)
                    .unwrap_or_else(|| format!("failed to call on_hook(): {e}"))
            };
            PluginError::Hook(msg)
        })?;

        match result {
            Ok(()) => Ok(()),
            Err(msg) => Err(PluginError::Hook(msg)),
        }
    }
}

/// Manages all loaded plugins and dispatches hook events.
///
/// Maintains a pre-built index from hook to registered plugins so
/// `fire_hook` and `has_hook` are O(1) lookups instead of iterating all plugins.
pub struct PluginManager {
    plugins: Vec<PluginRuntime>,
    /// Maps each hook to the indices of plugins registered for it.
    hook_index: HashMap<Hook, Vec<usize>>,
}

impl PluginManager {
    pub fn new(plugins: Vec<PluginRuntime>) -> Self {
        let mut hook_index: HashMap<Hook, Vec<usize>> = HashMap::new();
        for (i, plugin) in plugins.iter().enumerate() {
            for hook in &plugin.hooks {
                hook_index.entry(hook.clone()).or_default().push(i);
            }
        }
        Self {
            plugins,
            hook_index,
        }
    }

    /// Returns true if any loaded plugin is registered for the given hook.
    pub fn has_hook(&self, hook: &Hook) -> bool {
        self.hook_index.contains_key(hook)
    }

    /// Fire a hook synchronously across all plugins registered for it.
    ///
    /// Errors from individual plugins are logged but do not stop other plugins
    /// from receiving the event.
    pub fn fire_hook(&self, hook: Hook, values: Vec<(String, String)>) {
        let indices = match self.hook_index.get(&hook) {
            Some(indices) => indices,
            None => return,
        };

        let time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let event = HookEvent {
            hook: hook.clone(),
            time,
            target: hook_target(&hook),
            values,
        };

        if indices.len() <= 1 {
            for &i in indices {
                let plugin = &self.plugins[i];
                if let Err(e) = plugin.call_hook(&event) {
                    tracing::warn!(
                        plugin = %plugin.plugin_name,
                        hook = ?hook,
                        error = %e,
                        "Plugin hook failed"
                    );
                }
            }
        } else {
            // Multiple plugins: run in parallel with scoped threads.
            std::thread::scope(|s| {
                for &i in indices {
                    let plugin = &self.plugins[i];
                    let event = &event;
                    let hook = &hook;
                    s.spawn(move || {
                        if let Err(e) = plugin.call_hook(event) {
                            tracing::warn!(
                                plugin = %plugin.plugin_name,
                                hook = ?hook,
                                error = %e,
                                "Plugin hook failed"
                            );
                        }
                    });
                }
            });
        }
    }
}

/// Derive the target from a hook variant.
fn hook_target(hook: &Hook) -> HookTarget {
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
fn apply_permissions(
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
fn canonicalize_plugin_path(path: &std::path::Path) -> Result<std::path::PathBuf, PluginError> {
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
fn extract_panic_message(stderr: &str) -> Option<String> {
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

/// Replaces newlines and carriage returns so the message stays on a single log line.
fn sanitize_plugin_output(s: &str) -> String {
    s.replace('\n', "\\n").replace('\r', "\\r")
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
