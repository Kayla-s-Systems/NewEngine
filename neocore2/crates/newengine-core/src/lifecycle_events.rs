//! Engine lifecycle/readiness events.
//!
//! These events are intentionally small, cloneable DTOs. They are used by the
//! core dispatch/startup-graph path to make startup ordering explicit instead of
//! relying on ad-hoc "try now, maybe service exists" checks inside game/editor
//! modules.

use std::fmt;

/// Stable key used by the declarative startup graph.
///
/// Modules declare these keys through `Module::startup_requires()`. The engine
/// marks keys as satisfied when the corresponding lifecycle event is dispatched
/// and starts pending modules only after all of their declared keys are closed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EngineReadinessKey {
    /// Runtime/engine plugins were loaded and plugin-owned services are visible
    /// through the host service registry.
    EnginePluginsReady,

    /// Engine startup reached the post-plugin phase and startup dispatch
    /// completed. This is mostly useful for late observers; most gameplay
    /// systems should prefer a more specific readiness key.
    EngineStartCompleted,
}

impl EngineReadinessKey {
    #[inline]
    pub const fn as_str(self) -> &'static str {
        match self {
            EngineReadinessKey::EnginePluginsReady => "EnginePluginsReady",
            EngineReadinessKey::EngineStartCompleted => "EngineStartCompleted",
        }
    }
}

impl fmt::Display for EngineReadinessKey {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EngineLifecycleEvent {
    /// Runtime/engine plugins were loaded and their services are registered.
    ///
    /// Modules that require plugin-owned services should declare
    /// `EngineReadinessKey::EnginePluginsReady` instead of bootstrapping in
    /// `start()` based on load-order assumptions.
    EnginePluginsReady {
        loaded_count: usize,
        origin: &'static str,
    },

    /// All immediately startable modules completed `start()` and the engine is
    /// ready to tick. Modules may still be pending on future readiness keys.
    EngineStartCompleted {
        module_count: usize,
        plugin_count: usize,
    },
}

impl EngineLifecycleEvent {
    #[inline]
    pub const fn readiness_key(&self) -> EngineReadinessKey {
        match self {
            EngineLifecycleEvent::EnginePluginsReady { .. } => {
                EngineReadinessKey::EnginePluginsReady
            }
            EngineLifecycleEvent::EngineStartCompleted { .. } => {
                EngineReadinessKey::EngineStartCompleted
            }
        }
    }

    #[inline]
    pub const fn origin(&self) -> &'static str {
        match self {
            EngineLifecycleEvent::EnginePluginsReady { origin, .. } => origin,
            EngineLifecycleEvent::EngineStartCompleted { .. } => "engine-start-completed",
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct EngineReadinessSnapshot {
    pub engine_plugins_ready: bool,
    pub engine_start_completed: bool,
    pub plugin_count: usize,
    pub module_count: usize,
}

impl EngineReadinessSnapshot {
    #[inline]
    pub const fn is_satisfied(&self, key: EngineReadinessKey) -> bool {
        match key {
            EngineReadinessKey::EnginePluginsReady => self.engine_plugins_ready,
            EngineReadinessKey::EngineStartCompleted => self.engine_start_completed,
        }
    }
}
