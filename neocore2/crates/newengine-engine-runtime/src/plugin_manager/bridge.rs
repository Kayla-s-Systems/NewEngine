#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_math::collections_prelude::NeVecDeque as VecDeque;

use parking_lot::{Mutex, RwLock};

use newengine_plugin_host::PluginControlCommand;
use newengine_plugin_host::PluginsSnapshot;

/// UI <-> engine bridge for plugin diagnostics.
///
/// The engine publishes `PluginsSnapshot` every frame (via `Resources`).
/// The editor render/controller copies it here so the UI builder can render it
/// without requiring access to `ModuleCtx`.
#[derive(Debug, Default)]
pub struct PluginManagerBridge {
    snapshot: RwLock<PluginsSnapshot>,
    commands: Mutex<VecDeque<PluginControlCommand>>,
}

impl PluginManagerBridge {
    #[inline]
    pub fn new() -> Self {
        Self {
            snapshot: RwLock::new(PluginsSnapshot::default()),
            commands: Mutex::new(VecDeque::new()),
        }
    }

    #[inline]
    pub fn publish(&self, snap: PluginsSnapshot) {
        *self.snapshot.write() = snap;
    }

    #[inline]
    pub fn read(&self) -> PluginsSnapshot {
        self.snapshot.read().clone()
    }

    #[inline]
    pub fn push_cmd(&self, cmd: PluginControlCommand) {
        self.commands.lock().push_back(cmd);
    }

    #[inline]
    pub fn drain_cmds(&self) -> Vec<PluginControlCommand> {
        self.commands.lock().drain(..).collect()
    }
}
