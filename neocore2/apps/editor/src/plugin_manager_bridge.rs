#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_core::plugins::PluginsSnapshot;
use std::sync::RwLock;

/// UI <-> engine bridge for plugin diagnostics.
///
/// The engine publishes `PluginsSnapshot` every frame (via `Resources`).
/// The editor render/controller copies it here so the UI builder can render it
/// without requiring access to `ModuleCtx`.
#[derive(Debug, Default)]
pub struct PluginManagerBridge {
    snapshot: RwLock<PluginsSnapshot>,
}

impl PluginManagerBridge {
    #[inline]
    pub fn new() -> Self {
        Self {
            snapshot: RwLock::new(PluginsSnapshot::default()),
        }
    }

    #[inline]
    pub fn publish(&self, snap: PluginsSnapshot) {
        if let Ok(mut g) = self.snapshot.write() {
            *g = snap;
        }
    }

    #[inline]
    pub fn read(&self) -> PluginsSnapshot {
        self.snapshot.read().map(|g| g.clone()).unwrap_or_default()
    }
}
