#![forbid(unsafe_op_in_unsafe_fn)]

use super::module_slot::ModuleState;
use super::Engine;

use crate::lifecycle_events::{EngineLifecycleEvent, EngineReadinessKey, EngineReadinessSnapshot};
use crate::log_fmt::emit_prefixed_table;

/// Declarative startup readiness graph.
///
/// The graph stores only readiness facts. Module dependency ordering stays owned
/// by the module boot code; this graph decides whether a pending module may run
/// its `start()` hook yet.
#[derive(Debug, Clone, Default)]
pub(crate) struct StartupReadinessGraph {
    satisfied: Vec<EngineReadinessKey>,
}

impl StartupReadinessGraph {
    #[inline]
    pub(crate) fn is_satisfied(&self, key: EngineReadinessKey) -> bool {
        self.satisfied.iter().any(|&k| k == key)
    }

    #[inline]
    pub(crate) fn mark_satisfied(&mut self, key: EngineReadinessKey) -> bool {
        if self.is_satisfied(key) {
            return false;
        }
        self.satisfied.push(key);
        self.satisfied.sort();
        true
    }

    #[inline]
    pub(crate) fn all_satisfied(&self, requirements: &[EngineReadinessKey]) -> bool {
        requirements.iter().all(|&key| self.is_satisfied(key))
    }

    pub(crate) fn missing<'a>(
        &'a self,
        requirements: &'a [EngineReadinessKey],
    ) -> impl Iterator<Item = EngineReadinessKey> + 'a {
        requirements
            .iter()
            .copied()
            .filter(|&key| !self.is_satisfied(key))
    }

    #[inline]
    pub(crate) fn satisfied_csv(&self) -> String {
        readiness_csv(&self.satisfied)
    }
}

#[inline]
pub(crate) fn readiness_csv(keys: &[EngineReadinessKey]) -> String {
    if keys.is_empty() {
        return "-".to_string();
    }
    keys.iter()
        .map(|k| k.as_str())
        .collect::<Vec<_>>()
        .join(",")
}

impl<E: Send + 'static> Engine<E> {
    #[inline]
    pub(crate) fn mark_readiness_observed(&mut self, event: &EngineLifecycleEvent) {
        let key = event.readiness_key();
        let newly_satisfied = self.startup_graph.mark_satisfied(key);
        self.refresh_readiness_snapshot();

        if newly_satisfied {
            newengine_ulog_api::ulog::info!(
                "startup graph: readiness satisfied key='{}' origin='{}' satisfied='{}'",
                key,
                event.origin(),
                self.startup_graph.satisfied_csv(),
            );
        } else {
            newengine_ulog_api::ulog::debug!(
                "startup graph: readiness already satisfied key='{}' origin='{}' satisfied='{}'",
                key,
                event.origin(),
                self.startup_graph.satisfied_csv(),
            );
        }
    }

    #[inline]
    pub(crate) fn refresh_readiness_snapshot(&mut self) {
        let plugin_count = self.plugins.snapshot().len();
        let module_count = self
            .modules
            .iter()
            .filter(|s| s.state == ModuleState::Running)
            .count();

        self.resources.insert(EngineReadinessSnapshot {
            engine_plugins_ready: self
                .startup_graph
                .is_satisfied(EngineReadinessKey::EnginePluginsReady),
            engine_start_completed: self
                .startup_graph
                .is_satisfied(EngineReadinessKey::EngineStartCompleted),
            plugin_count,
            module_count,
        });
    }

    pub(crate) fn log_startup_graph_snapshot(&self, phase: &'static str) {
        let rows: Vec<Vec<String>> = self
            .modules
            .iter()
            .map(|slot| {
                let requirements = slot.module.startup_requires();
                let missing: Vec<EngineReadinessKey> =
                    self.startup_graph.missing(requirements).collect();
                vec![
                    slot.id().to_string(),
                    format!("{:?}", slot.state),
                    readiness_csv(requirements),
                    readiness_csv(&missing),
                ]
            })
            .collect();

        newengine_ulog_api::ulog::info!(
            "startup graph: phase='{}' satisfied='{}' modules={}",
            phase,
            self.startup_graph.satisfied_csv(),
            self.modules.len(),
        );

        if !rows.is_empty() && newengine_ulog_api::ulog::debug_enabled() {
            emit_prefixed_table(
                "[startup]",
                &format!("StartupGraph :: Modules [{}]", phase),
                &["module", "state", "requires", "missing"],
                &rows,
            );
        }
    }
}
