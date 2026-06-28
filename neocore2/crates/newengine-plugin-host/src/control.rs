#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_math::collections_prelude::NeVecDeque as VecDeque;
use std::path::PathBuf;

/// Host-side plugin control command.
///
/// This is an engine-only control plane intended for tooling (editor UI, telemetry)
/// to request actions without gaining direct access to `PluginManager`.
#[derive(Clone, Debug)]
pub enum PluginControlCommand {
    /// Scan the configured plugins directory and load any new dynamic libraries.
    Rescan,
    /// Load a plugin from an explicit dynamic library path.
    LoadPath(PathBuf),
    /// Reload a currently loaded plugin by id (unload + load from the same path).
    ReloadId(String),
    /// Start a plugin by id (valid for `registered` or `stopped`).
    StartId(String),
    /// Stop a plugin by id (best-effort shutdown + unregister).
    StopId(String),
    /// Disable a plugin by id (forces shutdown + unregister and marks disabled).
    DisableId(String),
    /// Enable a plugin by id.
    ///
    /// For safety this is implemented as `ReloadId` under the hood.
    EnableId(String),
}

/// Result of the last processed command batch.
#[derive(Clone, Debug, Default)]
pub struct PluginControlResult {
    pub last_action: Option<String>,
    pub last_error: Option<String>,
}

/// Engine-local queue for plugin control commands.
///
/// Stored in `Resources` and processed by the engine at frame boundaries.
#[derive(Debug, Default)]
pub struct PluginControlQueue {
    q: VecDeque<PluginControlCommand>,
    pub result: PluginControlResult,
}

impl PluginControlQueue {
    #[inline]
    pub fn push(&mut self, cmd: PluginControlCommand) {
        self.q.push_back(cmd);
    }

    #[inline]
    pub fn drain(&mut self) -> impl Iterator<Item = PluginControlCommand> + '_ {
        self.q.drain(..)
    }

    #[inline]
    pub fn clear_result(&mut self) {
        self.result = PluginControlResult::default();
    }
}
