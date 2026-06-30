use super::super::module_slot::ModuleState;
use super::super::{Engine, ModuleFaultTolerance};

use crate::error::{EngineError, EngineResult, ModuleStage};
use crate::module::ModuleCtx;
use crate::startup_status::{
    EngineStartupPhase, EngineStartupStepOutcome, EngineStartupStepPhase,
};

impl<E: Send + 'static> Engine<E> {
pub(super) fn start_incremental_init_module(&mut self) -> EngineResult<EngineStartupStepOutcome> {
    let total = self.modules.len();
    let mut index = self
        .incremental_startup
        .as_ref()
        .map(|s| s.index)
        .unwrap_or(0);

    while index < total {
        self.sync_shutdown_state();
        if self.is_shutdown_requested() {
            let err = EngineError::ExitRequested;
            return self.fail_incremental_startup(
                EngineStartupPhase::ModuleInit,
                "Startup interrupted by shutdown request.",
                "The shutdown token was requested while initializing modules.",
                self.module_init_progress(index, total),
                self.modules.get(index).map(|s| s.id().to_owned()),
                err,
            );
        }

        if self.modules[index].state != ModuleState::Pending {
            index += 1;
            if let Some(state) = &mut self.incremental_startup {
                state.index = index;
            }
            continue;
        }

        let module_id = self.modules[index].id();
        let progress_before = self.module_init_progress(index, total);
        let before = self.make_startup_snapshot(
            EngineStartupPhase::ModuleInit,
            format!("Initializing module {} of {}...", index + 1, total),
            format!("Calling init() for module '{module_id}'."),
            progress_before,
            Some(module_id.to_owned()),
            index,
            total,
        );
        self.publish_startup_snapshot(before);

        let init_started = std::time::Instant::now();
        let init_result = {
            let mut ctx = ModuleCtx::new(
                self.services.as_ref(),
                &mut self.resources,
                &self.bus,
                &self.events,
                &mut self.scheduler,
                self.shutdown.clone(),
            );
            self.modules[index].module.init(&mut ctx)
        };
        let init_ms = init_started.elapsed().as_millis();

        match init_result {
            Ok(()) => {
                newengine_ulog_api::ulog::info!(
                    "startup fsm: module init complete module='{}' index={} total={} elapsed_ms={}",
                    module_id,
                    index + 1,
                    total,
                    init_ms
                );
                if let Some(state) = &mut self.incremental_startup {
                    state.initialized = state.initialized.saturating_add(1);
                    state.index = index + 1;
                }
                let done = index + 1;
                let snapshot = self.make_startup_snapshot(
                    EngineStartupPhase::ModuleInit,
                    format!("Module initialized ({done}/{total})."),
                    format!("Module '{module_id}' completed init()."),
                    self.module_init_progress(done, total),
                    Some(module_id.to_owned()),
                    done,
                    total,
                );
                self.publish_startup_snapshot(snapshot.clone());
                if done >= total {
                    self.set_incremental_phase(EngineStartupStepPhase::StartupGraphInitial);
                }
                return Ok(EngineStartupStepOutcome::running(snapshot));
            }
            Err(err) => {
                match self.module_fault_tolerance {
                    ModuleFaultTolerance::Strict => {
                        newengine_ulog_api::ulog::error!(
                        "startup fsm: module init failed module='{}' index={} total={} elapsed_ms={} tolerance=strict err='{}'",
                        module_id,
                        index + 1,
                        total,
                        init_ms,
                        err
                    );
                        let initialized = self
                            .incremental_startup
                            .as_ref()
                            .map(|s| s.initialized)
                            .unwrap_or(index);
                        self.shutdown_initialized_modules(initialized);
                        return self.fail_incremental_startup(
                        EngineStartupPhase::ModuleInit,
                        format!("Module init failed: {module_id}"),
                        format!("Strict startup stopped while initializing module '{module_id}'."),
                        progress_before,
                        Some(module_id.to_owned()),
                        EngineError::with_module_stage(module_id, ModuleStage::Init, err),
                    );
                    }
                    ModuleFaultTolerance::Resilient => {
                        let reason = format!("init failed: {err}");
                        newengine_ulog_api::ulog::error!(
                        "startup fsm: module init failed module='{}' index={} total={} elapsed_ms={} tolerance=resilient err='{}'",
                        module_id,
                        index + 1,
                        total,
                        init_ms,
                        err
                    );
                        self.modules[index].disable(reason.clone());
                        self.shutdown_slot_by_index(index);
                        index += 1;
                        if let Some(state) = &mut self.incremental_startup {
                            state.index = index;
                        }
                        let snapshot = self.make_startup_snapshot(
                            EngineStartupPhase::ModuleInit,
                            format!("Module disabled: {module_id}"),
                            format!("Resilient startup disabled '{module_id}': {reason}"),
                            self.module_init_progress(index, total),
                            Some(module_id.to_owned()),
                            index,
                            total,
                        );
                        self.publish_startup_snapshot(snapshot.clone());
                        return Ok(EngineStartupStepOutcome::running(snapshot));
                    }
                }
            }
        }
    }

    self.set_incremental_phase(EngineStartupStepPhase::StartupGraphInitial);
    let snapshot = self.make_startup_snapshot(
        EngineStartupPhase::ModuleInit,
        "Module initialization complete.",
        format!("{} module slot(s) processed.", total),
        0.66,
        None,
        total,
        total,
    );
    self.publish_startup_snapshot(snapshot.clone());
    Ok(EngineStartupStepOutcome::running(snapshot))
}
pub(super) fn shutdown_initialized_modules(&mut self, initialized: usize) {
    let end = initialized.min(self.modules.len());
    for i in (0..end).rev() {
        self.shutdown_slot_by_index(i);
    }
}
pub(super) fn shutdown_slot_by_index(&mut self, index: usize) {
    if self.modules[index].shutdown_called {
        return;
    }
    let mut ctx = ModuleCtx::new(
        self.services.as_ref(),
        &mut self.resources,
        &self.bus,
        &self.events,
        &mut self.scheduler,
        self.shutdown.clone(),
    );
    let _ = self.modules[index].module.shutdown(&mut ctx);
    self.modules[index].shutdown_called = true;
    self.modules[index].state = ModuleState::Disabled;
}
}
