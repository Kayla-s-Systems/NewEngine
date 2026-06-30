#![forbid(unsafe_op_in_unsafe_fn)]

mod diagnostics;
mod load;
mod control;

use super::Engine;

use crate::error::{EngineError, EngineResult};
use newengine_plugin_host::PluginsSnapshot;

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
