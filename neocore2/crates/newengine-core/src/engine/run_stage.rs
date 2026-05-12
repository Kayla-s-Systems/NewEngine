#![forbid(unsafe_op_in_unsafe_fn)]

use super::module_slot::ModuleState;
use super::{Engine, EngineRunState, ModuleFaultTolerance};

use crate::error::{EngineError, EngineResult, ModuleStage};
use crate::module::{Module, ModuleCtx};

use std::panic::{self, AssertUnwindSafe};

impl<E: Send + 'static> Engine<E> {
    #[inline]
    pub(super) fn run_stage<F>(
        &mut self,
        frame: &crate::frame::Frame,
        stage: ModuleStage,
        mut call: F,
    ) -> EngineResult<()>
    where
        F: FnMut(&mut dyn Module<E>, &mut ModuleCtx<'_, E>) -> EngineResult<()>,
    {
        self.sync_shutdown_state();
        if self.is_shutdown_requested() {
            return Err(EngineError::ExitRequested);
        }

        let services = self.services.as_ref();
        let bus = &self.bus;
        let events = &self.events;
        let shutdown = &self.shutdown;

        let resources = &mut self.resources;
        let scheduler = &mut self.scheduler;

        for s in self.modules.iter_mut() {
            if s.state != ModuleState::Running {
                continue;
            }

            if shutdown.is_requested() {
                return Err(EngineError::ExitRequested);
            }

            let module_id = s.id();

            let result: EngineResult<()> = {
                let mut ctx = ModuleCtx::new(services, resources, bus, events, scheduler, shutdown.clone());
                ctx.set_frame(frame);

                if self.catch_panics {
                    match panic::catch_unwind(AssertUnwindSafe(|| call(s.module.as_mut(), &mut ctx))) {
                        Ok(r) => r,
                        Err(payload) => Err(EngineError::Other(format!(
                            "panic in module callback (module='{module_id}' stage={stage:?} msg='{}')",
                            Self::panic_message(payload)
                        ))),
                    }
                } else {
                    call(s.module.as_mut(), &mut ctx)
                }
            };

            if let Err(e) = result {
                match self.module_fault_tolerance {
                    ModuleFaultTolerance::Strict => {
                        shutdown.request();
                        return Err(EngineError::with_module_stage(module_id, stage, e));
                    }
                    ModuleFaultTolerance::Resilient => {
                        let reason = format!("stage {stage:?} failed: {e}");
                        log::error!("engine: disabling module {} ({})", module_id, reason);

                        s.disable(reason);

                        if !s.shutdown_called {
                            let mut ctx =
                                ModuleCtx::new(services, resources, bus, events, scheduler, shutdown.clone());
                            ctx.set_frame(frame);

                            let _ = s.module.shutdown(&mut ctx);
                            s.shutdown_called = true;
                            s.state = ModuleState::Disabled;
                        }

                        continue;
                    }
                }
            }

            if shutdown.is_requested() {
                return Err(EngineError::ExitRequested);
            }
        }

        Ok(())
    }

    pub fn shutdown(&mut self) -> EngineResult<()> {
        self.sync_shutdown_state();
        self.set_run_state(EngineRunState::ShutdownGame);

        // Modules own engine-side handles into plugin services. They must be allowed
        // to close frames, unregister APIs and release logical resources before the
        // plugin host tears down the actual service objects/DLL-backed state.
        for s in self.modules.iter_mut().rev() {
            if s.shutdown_called {
                continue;
            }

            let module_id = s.id();

            let mut ctx = ModuleCtx::new(
                self.services.as_ref(),
                &mut self.resources,
                &self.bus,
                &self.events,
                &mut self.scheduler,
                self.shutdown.clone(),
            );

            log::debug!("engine shutdown: module shutdown begin id='{}'", module_id);
            crate::crash::record_breadcrumb(format!(
                "engine shutdown: module shutdown begin id={module_id}"
            ));
            let _ = s
                .module
                .shutdown(&mut ctx)
                .map_err(|e| EngineError::with_module_stage(module_id, ModuleStage::Shutdown, e));

            log::debug!("engine shutdown: module shutdown completed id='{}'", module_id);
            crate::crash::record_breadcrumb(format!(
                "engine shutdown: module shutdown completed id={module_id}"
            ));
            s.shutdown_called = true;
            s.state = ModuleState::Disabled;
        }

        self.set_run_state(EngineRunState::ShutdownSystem);
        self.job_system.shutdown_and_join();
        self.plugins_shutdown();
        self.set_run_state(EngineRunState::Stopped);

        Ok(())
    }
}