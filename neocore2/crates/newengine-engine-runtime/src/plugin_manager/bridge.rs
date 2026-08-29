#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_math::collections_prelude::NeVecDeque as VecDeque;

use parking_lot::{Mutex, RwLock};
use std::sync::atomic::{AtomicU64, Ordering};

use newengine_plugin_host::PluginControlCommand;
use newengine_plugin_host::PluginsSnapshot;

/// UI <-> engine bridge for plugin diagnostics.
///
/// The engine publishes `PluginsSnapshot` only when plugin state changes.
/// The bridge uses the snapshot revision as a retained-state gate, so normal render
/// frames do not rewrite the `RwLock` or clone plugin entry vectors.
#[derive(Debug)]
pub struct PluginManagerBridge {
    published_revision: AtomicU64,
    snapshot: RwLock<PluginsSnapshot>,
    commands: Mutex<VecDeque<PluginControlCommand>>,
}

impl Default for PluginManagerBridge {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl PluginManagerBridge {
    #[inline]
    pub fn new() -> Self {
        Self {
            published_revision: AtomicU64::new(0),
            snapshot: RwLock::new(PluginsSnapshot::default()),
            commands: Mutex::new(VecDeque::new()),
        }
    }

    #[inline]
    pub fn publish_if_changed(&self, snap: &PluginsSnapshot) -> bool {
        if self.published_revision.load(Ordering::Acquire) == snap.revision {
            return false;
        }
        *self.snapshot.write() = snap.clone();
        self.published_revision
            .store(snap.revision, Ordering::Release);
        true
    }

    #[inline]
    pub fn revision(&self) -> u64 {
        self.published_revision.load(Ordering::Acquire)
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
