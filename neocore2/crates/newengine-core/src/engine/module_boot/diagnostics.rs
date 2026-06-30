use super::super::{Engine, EngineRunState};

use crate::error::{EngineError, EngineResult};
use crate::startup_status::{
    EngineStartupPhase, EngineStartupSnapshot, EngineStartupStepOutcome, EngineStartupSystemPhase,
    EngineStartupSystemStatus,
};

impl<E: Send + 'static> Engine<E> {
    #[inline]
    pub(super) fn module_init_progress(&self, done: usize, total: usize) -> f32 {
        if total == 0 {
            0.66
        } else {
            0.32 + (done as f32 / total as f32).clamp(0.0, 1.0) * 0.34
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn make_startup_snapshot(
        &self,
        phase: EngineStartupPhase,
        status: impl Into<String>,
        detail: impl Into<String>,
        progress_01: f32,
        current_module: Option<String>,
        module_index: usize,
        module_total: usize,
    ) -> EngineStartupSnapshot {
        let plugin_count = self.plugins.snapshot().len();
        let systems = self.make_startup_systems(
            phase,
            progress_01,
            current_module.as_deref(),
            module_index,
            module_total,
            plugin_count,
        );

        EngineStartupSnapshot::running(
            phase,
            self.run_state().as_str(),
            status,
            detail,
            progress_01,
            current_module,
            module_index,
            module_total,
            plugin_count,
            systems,
        )
    }

    pub(super) fn make_startup_systems(
        &self,
        phase: EngineStartupPhase,
        progress_01: f32,
        current_module: Option<&str>,
        module_index: usize,
        module_total: usize,
        plugin_count: usize,
    ) -> Vec<EngineStartupSystemStatus> {
        let modules_phase =
            startup_system_phase(phase == EngineStartupPhase::ModuleInit, progress_01, 0.70);
        let plugins_phase = startup_system_phase(
            matches!(
                phase,
                EngineStartupPhase::RuntimePlugins | EngineStartupPhase::PluginStart
            ),
            progress_01,
            0.88,
        );
        let contracts_phase = startup_system_phase(
            phase == EngineStartupPhase::ServiceContracts,
            progress_01,
            0.91,
        );
        let readiness_phase = startup_system_phase(
            phase == EngineStartupPhase::ReadinessEvents,
            progress_01,
            0.94,
        );

        vec![
            EngineStartupSystemStatus::new(
                "fsm",
                "CORE FSM",
                if self.run_state().is_booting() {
                    EngineStartupSystemPhase::Running
                } else {
                    EngineStartupSystemPhase::Ready
                },
                self.run_state().as_str().to_ascii_uppercase(),
                format!("Core lifecycle state is '{}'.", self.run_state().as_str()),
                Some(progress_01),
            ),
            EngineStartupSystemStatus::new(
                "modules",
                "MODULES",
                modules_phase,
                startup_system_state_label(modules_phase, "INIT"),
                current_module
                    .map(|m| format!("Processing module '{m}' ({module_index}/{module_total})."))
                    .unwrap_or_else(|| format!("{} module slot(s) registered.", self.modules.len())),
                (module_total > 0).then_some(
                    (module_index as f32 / module_total as f32).clamp(0.0, 1.0),
                ),
            ),
            EngineStartupSystemStatus::new(
                "plugins",
                "PLUGINS",
                plugins_phase,
                startup_system_state_label(plugins_phase, "LOAD"),
                format!("{plugin_count} plugin descriptor(s) known to the host."),
                Some(progress_until_ready(progress_01, 0.88)),
            ),
            EngineStartupSystemStatus::new(
                "contracts",
                "CONTRACTS",
                contracts_phase,
                startup_system_state_label(contracts_phase, "CHECK"),
                "Runtime service contracts are validated before readiness events reach gameplay modules.",
                Some(progress_until_ready(progress_01, 0.91)),
            ),
            EngineStartupSystemStatus::new(
                "readiness",
                "READINESS",
                readiness_phase,
                startup_system_state_label(readiness_phase, "EVENTS"),
                "Startup graph readiness facts are being collected and dispatched.",
                Some(progress_until_ready(progress_01, 0.94)),
            ),
            EngineStartupSystemStatus::new(
                "diagnostics",
                "DIAGNOSTICS",
                EngineStartupSystemPhase::Running,
                phase.human_label(),
                format!(
                    "phase='{}' run_state='{}'",
                    phase.as_str(),
                    self.run_state().as_str()
                ),
                Some(progress_01),
            ),
        ]
    }

    pub(super) fn publish_startup_snapshot(&mut self, snapshot: EngineStartupSnapshot) {
        self.resources.insert(snapshot.clone());
        let _ = self.events.publish(snapshot.clone());
        self.startup_snapshot = snapshot;
    }

    pub(super) fn fail_incremental_startup(
        &mut self,
        phase: EngineStartupPhase,
        status: impl Into<String>,
        detail: impl Into<String>,
        progress_01: f32,
        current_module: Option<String>,
        err: EngineError,
    ) -> EngineResult<EngineStartupStepOutcome> {
        if !self.run_state().is_terminal() {
            self.set_run_state(EngineRunState::Faulted);
        }
        let error = err.to_string();
        let current_module_for_log = current_module.as_deref().unwrap_or("-");
        newengine_ulog_api::ulog::error!(
            "startup fsm: failed phase='{}' run_state='{}' module='{}' progress={:.2} err='{}'",
            phase.as_str(),
            self.run_state().as_str(),
            current_module_for_log,
            progress_01,
            error
        );
        let module_index = self
            .incremental_startup
            .as_ref()
            .map(|s| s.index)
            .unwrap_or(0);
        let module_total = self.modules.len();
        let snapshot = EngineStartupSnapshot::failed(
            phase,
            self.run_state().as_str(),
            status,
            detail,
            progress_01,
            current_module,
            module_index,
            module_total,
            self.plugins.snapshot().len(),
            error,
        );
        self.publish_startup_snapshot(snapshot);
        Err(err)
    }
}

#[inline]
fn startup_system_phase(
    running: bool,
    progress_01: f32,
    ready_at: f32,
) -> EngineStartupSystemPhase {
    if running {
        EngineStartupSystemPhase::Running
    } else if progress_01 >= ready_at {
        EngineStartupSystemPhase::Ready
    } else {
        EngineStartupSystemPhase::Waiting
    }
}

#[inline]
fn startup_system_state_label(
    phase: EngineStartupSystemPhase,
    running_label: &'static str,
) -> &'static str {
    match phase {
        EngineStartupSystemPhase::Waiting => "WAIT",
        EngineStartupSystemPhase::Running => running_label,
        EngineStartupSystemPhase::Ready => "READY",
        EngineStartupSystemPhase::Degraded => "DEGRADED",
        EngineStartupSystemPhase::Failed => "ERR",
    }
}

#[inline]
fn progress_until_ready(progress_01: f32, ready_at: f32) -> f32 {
    if progress_01 >= ready_at {
        1.0
    } else {
        progress_01.clamp(0.0, 1.0)
    }
}
