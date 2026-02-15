use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use super::helpers::hook_target;
use super::runtime::PluginRuntime;
use super::{Hook, HookEvent};

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
            for hook in plugin.hooks() {
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

    /// Fire a hook asynchronously across all plugins registered for it.
    ///
    /// Different plugins run concurrently via `tokio::join!`. A single plugin
    /// serializes its hook calls via `tokio::sync::Mutex` (WASM is single-threaded).
    /// Errors from individual plugins are logged but do not stop other plugins
    /// from receiving the event.
    pub async fn fire_hook(&self, hook: Hook, values: Vec<(String, String)>) {
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

        let futures: Vec<_> = indices
            .iter()
            .map(|&i| {
                let plugin = &self.plugins[i];
                let event = &event;
                let hook = &hook;
                async move {
                    if let Err(e) = plugin.call_hook(event).await {
                        tracing::warn!(
                            plugin = %plugin.name(),
                            hook = ?hook,
                            error = %e,
                            "Plugin hook failed"
                        );
                    }
                }
            })
            .collect();

        futures::future::join_all(futures).await;
    }
}
