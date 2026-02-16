#![forbid(unsafe_op_in_unsafe_fn)]

use std::collections::VecDeque;
use std::sync::{Mutex, RwLock};

use newengine_core::plugins::PluginControlCommand;
use newengine_core::plugins::PluginsSnapshot;

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
        if let Ok(mut g) = self.snapshot.write() {
            *g = snap;
        }
    }

    #[inline]
    pub fn read(&self) -> PluginsSnapshot {
        self.snapshot.read().map(|g| g.clone()).unwrap_or_default()
    }

    #[inline]
    pub fn push_cmd(&self, cmd: PluginControlCommand) {
        if let Ok(mut q) = self.commands.lock() {
            q.push_back(cmd);
        }
    }

    #[inline]
    pub fn drain_cmds(&self) -> Vec<PluginControlCommand> {
        let Ok(mut q) = self.commands.lock() else {
            return Vec::new();
        };
        q.drain(..).collect()
    }
}
