use super::super::{Engine, EngineRunState};

use crate::error::{EngineError, EngineResult};
use crate::startup_status::EngineStartupStepPhase;

impl<E: Send + 'static> Engine<E> {
#[inline]
pub(super) fn set_incremental_phase(&mut self, phase: EngineStartupStepPhase) {
    let Some(previous_state) = self.incremental_startup.as_ref() else {
        return;
    };
    let previous = previous_state.phase;
    let initialized = previous_state.initialized;
    if previous != phase {
        newengine_ulog_api::ulog::debug!(
            "startup fsm: phase {} -> {} run_state='{}' modules={}/{} plugins={}",
            previous.as_str(),
            phase.as_str(),
            self.run_state().as_str(),
            initialized,
            self.modules.len(),
            self.plugins.snapshot().len()
        );
    }
    if let Some(state) = &mut self.incremental_startup {
        state.phase = phase;
    }
}


#[inline]
pub(super) fn enter_system_init(&mut self) -> EngineResult<()> {
    self.set_run_state(EngineRunState::InitSystem);
    self.last = std::time::Instant::now();
    self.sync_shutdown_state();

    if self.is_shutdown_requested() {
        return Err(EngineError::ExitRequested);
    }

    Ok(())
}

#[inline]
pub(super) fn enter_game_init(&mut self) {
    self.set_run_state(EngineRunState::InitGame);
}
}
