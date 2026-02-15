use std::path::{Path, PathBuf};
use std::time::Duration;

use tokio::sync::Mutex;
use wasmtime::component::{Component, HasSelf, Linker};
use wasmtime::{Engine, Store, StoreLimitsBuilder};
use wasmtime_wasi::WasiCtxBuilder;
use wasmtime_wasi::p2::pipe::MemoryOutputPipe;

use super::helpers::{apply_permissions, extract_panic_message, hook_target};
use super::permissions::PluginPermission;
use super::state::PluginState;
use super::{Hook, HookEvent, HookTarget, Plugin, PluginError};

/// Stderr buffer capacity in bytes. Used only for capturing panic messages
/// from the WASM guest. The buffer has a hard cap — once full, further writes
/// trap the plugin. Since we only read stderr on errors, this is acceptable:
/// if a plugin panics repeatedly and fills the buffer, later calls simply
/// won't include panic details (the wasmtime error message is still present).
const STDERR_CAPACITY: usize = 4096;

/// How often (in fuel units) WASM yields to the tokio runtime during execution.
/// This ensures `tokio::time::timeout` can fire even during pure-compute loops,
/// because without yielding the timeout future never gets polled.
const FUEL_YIELD_INTERVAL: u64 = 10_000;

/// A loaded and validated WASM plugin.
///
/// `PluginRuntime` holds the metadata extracted from a plugin's `config()` export
/// after successfully loading, compiling, and instantiating the WASM component.
///
/// # Lifecycle
///
/// 1. **Read** — The `.wasm` file is read from disk.
/// 2. **Compile** — The bytes are compiled into a wasmtime `Component`.
/// 3. **Link** — WASI imports and the host `log()` function are wired up.
///    Stderr is captured for panic extraction; filesystem, env, and network
///    access is granted per-plugin via permissions.
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
    /// On timeout, the old instance is dropped and a fresh one is created.
    instance: Mutex<Option<PluginInstance>>,
    /// Wall-clock timeout for `config()` and `on-hook()` calls.
    /// Covers both WASM execution and time spent in async host calls.
    hook_timeout: Duration,
    /// Retained for reloading after a timeout kills the instance.
    path: PathBuf,
    permissions: Vec<PluginPermission>,
    config_vars: Vec<(String, String)>,
}

/// A live WASM plugin instance: store, bindings, and stderr pipe.
struct PluginInstance {
    store: Store<PluginState>,
    plugin: Plugin,
    /// Stderr pipe shared with the WASI context. We snapshot its length before
    /// each call and read only new bytes afterward, so stale data from previous
    /// calls is ignored. The pipe has a hard capacity cap (see `STDERR_CAPACITY`).
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
    pub async fn load(
        path: &Path,
        permissions: &[PluginPermission],
        config_vars: &[(String, String)],
        hook_timeout: Duration,
    ) -> Result<Self, PluginError> {
        let (inst, config) =
            Self::create_instance(path, permissions, config_vars, hook_timeout).await?;

        // Validate the config — a plugin must have a non-empty name.
        if config.name.is_empty() {
            return Err(PluginError::InvalidConfig("plugin name is empty".into()));
        }

        // Validate that all hooks match the declared target.
        for hook in &config.hooks {
            let target = hook_target(hook);
            if target != config.target {
                return Err(PluginError::InvalidConfig(format!(
                    "hook {hook:?} has target {target:?} but plugin declared target {:?}",
                    config.target
                )));
            }
        }

        Ok(Self {
            plugin_name: config.name,
            plugin_version: config.version,
            target: config.target,
            hooks: config.hooks,
            instance: Mutex::new(Some(inst)),
            hook_timeout,
            path: path.to_path_buf(),
            permissions: permissions.to_vec(),
            config_vars: config_vars.to_vec(),
        })
    }

    /// Create a fresh WASM instance: compile, link, instantiate, call `config()`.
    ///
    /// Used by both initial `load()` and automatic reload after a timeout.
    /// Returns the live instance and the plugin config from `config()`.
    async fn create_instance(
        path: &Path,
        permissions: &[PluginPermission],
        config_vars: &[(String, String)],
        hook_timeout: Duration,
    ) -> Result<(PluginInstance, super::PluginConfig), PluginError> {
        // Read the raw .wasm bytes from disk.
        let wasm_bytes = std::fs::read(path)
            .map_err(|e| PluginError::Load(format!("failed to read {}: {e}", path.display())))?;

        // Create a wasmtime engine with resource limits and compile the bytes
        // into a Component. Fuel metering caps CPU usage.
        let mut engine_config = wasmtime::Config::new();
        engine_config.consume_fuel(true);
        engine_config.async_support(true);
        engine_config.max_wasm_stack(512 * 1024); // 512KB stack limit
        let engine = Engine::new(&engine_config)
            .map_err(|e| PluginError::Load(format!("failed to create engine: {e}")))?;

        let component = Component::new(&engine, &wasm_bytes)
            .map_err(|e| PluginError::Load(format!("failed to compile {}: {e}", path.display())))?;

        // Set up the linker with WASI imports and our custom `log` import.
        let mut linker: Linker<PluginState> = Linker::new(&engine);
        wasmtime_wasi::p2::add_to_linker_async(&mut linker)
            .map_err(|e| PluginError::Load(format!("failed to add WASI to linker: {e}")))?;
        Plugin::add_to_linker::<_, HasSelf<_>>(&mut linker, |state| state).map_err(|e| {
            PluginError::Load(format!("failed to add plugin imports to linker: {e}"))
        })?;

        // Build the WASI context. Stderr is captured to read panic messages.
        let stderr = MemoryOutputPipe::new(STDERR_CAPACITY);
        let mut wasi_builder = WasiCtxBuilder::new();
        wasi_builder.stderr(stderr.clone());
        apply_permissions(&mut wasi_builder, permissions)?;
        let wasi = wasi_builder.build();

        // Derive a logging name from the file path for use before config() returns.
        let plugin_name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();

        // Memory limits: cap each linear memory at 10MB to prevent OOM.
        let limits = StoreLimitsBuilder::new()
            .memory_size(10 * 1024 * 1024)
            .build();
        let mut store = Store::new(
            &engine,
            PluginState {
                wasi,
                table: wasmtime_wasi::ResourceTable::new(),
                limits,
                plugin_name,
            },
        );
        store.limiter(|state| &mut state.limits);

        // Yield to tokio periodically so wall-clock timeouts can fire
        // even during pure-compute WASM loops.
        store
            .fuel_async_yield_interval(Some(FUEL_YIELD_INTERVAL))
            .map_err(|e| PluginError::Load(format!("failed to set yield interval: {e}")))?;

        // Fuel limit: ~10M instructions for the config() call.
        store
            .set_fuel(10_000_000)
            .map_err(|e| PluginError::Load(format!("failed to set fuel limit: {e}")))?;

        // Instantiate the component asynchronously.
        let instance = Plugin::instantiate_async(&mut store, &component, &linker)
            .await
            .map_err(|e| PluginError::Runtime(format!("failed to instantiate plugin: {e}")))?;

        // Call the plugin's exported `config()` function to retrieve its metadata.
        // Wrapped in a wall-clock timeout to catch plugins that block on host calls.
        let stderr_offset = stderr.contents().len();
        let config =
            tokio::time::timeout(hook_timeout, instance.call_config(&mut store, config_vars))
                .await
                .map_err(|_| {
                    PluginError::Runtime(format!(
                        "config() timed out after {}ms",
                        hook_timeout.as_millis()
                    ))
                })?
                .map_err(|e| {
                    let stderr_output = read_new_stderr(&stderr, stderr_offset);
                    let msg = format_trap_error("config()", &e, &stderr_output);
                    PluginError::Runtime(msg)
                })?;

        // Refuel the store for future hook calls (config() consumed some fuel).
        store
            .set_fuel(10_000_000)
            .map_err(|e| PluginError::Load(format!("failed to reset fuel: {e}")))?;

        // Update the plugin name in the store to the declared name from config().
        store.data_mut().plugin_name = config.name.clone();

        let inst = PluginInstance {
            store,
            plugin: instance,
            stderr,
        };
        Ok((inst, config))
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
    /// call to ensure the plugin has a fresh CPU budget. Wrapped in a wall-clock
    /// timeout to catch plugins that block on async host calls (network, filesystem).
    ///
    /// If the call times out, the old instance is dropped (its store is in an
    /// undefined state) and a fresh instance is created from disk. The timeout
    /// error is still returned for this call.
    pub async fn call_hook(&self, event: &HookEvent) -> Result<(), PluginError> {
        let mut guard = self.instance.lock().await;

        // Reload if a previous timeout left us with no instance.
        if guard.is_none() {
            match Self::create_instance(
                &self.path,
                &self.permissions,
                &self.config_vars,
                self.hook_timeout,
            )
            .await
            {
                Ok((inst, _config)) => {
                    tracing::info!(plugin = %self.plugin_name, "Plugin reloaded after timeout");
                    *guard = Some(inst);
                }
                Err(e) => {
                    tracing::warn!(
                        plugin = %self.plugin_name,
                        error = %e,
                        "Failed to reload plugin after timeout"
                    );
                    return Err(PluginError::Hook(format!("plugin reload failed: {e}")));
                }
            }
        }

        let inst = guard.as_mut().unwrap();

        // Refuel before each hook call.
        inst.store
            .set_fuel(10_000_000)
            .map_err(|e| PluginError::Hook(format!("failed to set fuel limit: {e}")))?;

        // Snapshot stderr length so we only read new bytes on error.
        let stderr_offset = inst.stderr.contents().len();

        let timeout = self.hook_timeout;
        let result =
            tokio::time::timeout(timeout, inst.plugin.call_on_hook(&mut inst.store, event)).await;

        if result.is_err() {
            // Timeout fired — the future was dropped mid-execution, leaving the
            // store in an undefined state. Drop the instance; it will be reloaded
            // from disk on the next call.
            *guard = None;
            return Err(PluginError::Hook(format!(
                "on_hook() timed out after {}ms, plugin will reload on next call",
                timeout.as_millis()
            )));
        }

        let result = result.unwrap().map_err(|e| {
            let stderr_output = read_new_stderr(&inst.stderr, stderr_offset);
            let msg = format_trap_error("on_hook()", &e, &stderr_output);
            PluginError::Hook(msg)
        });

        match result {
            Ok(Ok(())) => Ok(()),
            Ok(Err(msg)) => Err(PluginError::Hook(msg)),
            Err(e) => Err(e),
        }
    }
}

/// Read only the new bytes written to stderr since `offset`.
fn read_new_stderr(stderr: &MemoryOutputPipe, offset: usize) -> String {
    let all = stderr.contents();
    let new_bytes = &all[offset.min(all.len())..];
    String::from_utf8_lossy(new_bytes).into_owned()
}

/// Format a trap error with optional stderr context.
///
/// In verbose mode (`RUST_BACKTRACE` set), includes the full stderr dump.
/// Otherwise, tries to extract just the panic message for a cleaner error.
fn format_trap_error(call_name: &str, error: &wasmtime::Error, stderr_output: &str) -> String {
    let verbose = std::env::var_os("RUST_BACKTRACE").is_some();
    if verbose {
        let mut msg = format!("failed to call {call_name}: {error}");
        if !stderr_output.is_empty() {
            msg = format!("{msg}\n\nplugin stderr:\n{stderr_output}");
        }
        msg
    } else {
        extract_panic_message(stderr_output)
            .unwrap_or_else(|| format!("failed to call {call_name}: {error}"))
    }
}
