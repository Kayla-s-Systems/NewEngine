#![forbid(unsafe_op_in_unsafe_fn)]

mod control;
mod diagnostics;
mod load;

use super::Engine;

use crate::error::{EngineError, EngineResult};
use newengine_plugin_host::PluginsSnapshot;

#[inline]
fn bootstrap_preload_deferred() -> bool {
    std::env::var("NEWENGINE_BOOTSTRAP_PLUGIN_PRELOAD")
        .ok()
        .map(|v| {
            matches!(
                v.trim().to_ascii_lowercase().as_str(),
                "0" | "false" | "off" | "defer" | "deferred" | "safe"
            )
        })
        .unwrap_or(false)
}

impl<E: Send + 'static> Engine<E> {
    #[inline]
    pub(crate) fn expose_plugins_snapshot(&mut self) {
        self.resources.insert(PluginsSnapshot {
            plugins: self.plugins.snapshot(),
        });
    }

    #[inline]
    pub(crate) fn plugins_start_all(&mut self) -> EngineResult<()> {
        self.plugins
            .start_all()
            .map_err(|e| EngineError::Other(format!("plugins: start failed: {e}")))
    }

    #[inline]
    pub(crate) fn plugins_shutdown(&mut self) {
        self.plugins.shutdown();
    }
}
