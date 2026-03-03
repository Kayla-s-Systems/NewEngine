#![forbid(unsafe_op_in_unsafe_fn)]

use super::module_slot::ModuleState;
use super::{Engine, ModuleFaultTolerance};

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
        if self.is_exit_requested() {
            return Err(EngineError::ExitRequested);
        }

        let services = self.services.as_ref();
        let bus = &self.bus;
        let events = &self.events;
        let shutdown = &self.shutdown;

        let resources = &mut self.resources;
        let scheduler = &mut self.scheduler;
        let exit_requested = &mut self.exit_requested;

        for s in self.modules.iter_mut() {
            if s.state != ModuleState::Running {
                continue;
            }

            if shutdown.is_requested() {
                *exit_requested = true;
            }
            if *exit_requested {
                shutdown.request();
                return Err(EngineError::ExitRequested);
            }

            let module_id = s.id();

            let result: EngineResult<()> = {
                let mut ctx = ModuleCtx::new(services, resources, bus, events, scheduler, exit_requested);
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
                        *exit_requested = true;
                        shutdown.request();
                        return Err(EngineError::with_module_stage(module_id, stage, e));
                    }
                    ModuleFaultTolerance::Resilient => {
                        let reason = format!("stage {stage:?} failed: {e}");
                        log::error!("engine: disabling module {} ({})", module_id, reason);

                        s.disable(reason);

                        if !s.shutdown_called {
                            let mut ctx =
                                ModuleCtx::new(services, resources, bus, events, scheduler, exit_requested);
                            ctx.set_frame(frame);

                            let _ = s.module.shutdown(&mut ctx);
                            s.shutdown_called = true;
                            s.state = ModuleState::Disabled;
                        }

                        continue;
                    }
                }
            }

            if *exit_requested {
                shutdown.request();
                return Err(EngineError::ExitRequested);
            }
        }

        Ok(())
    }

    pub fn shutdown(&mut self) -> EngineResult<()> {
        self.sync_shutdown_state();

        self.plugins_shutdown();

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
                &mut self.exit_requested,
            );

            let _ = s
                .module
                .shutdown(&mut ctx)
                .map_err(|e| EngineError::with_module_stage(module_id, ModuleStage::Shutdown, e));

            s.shutdown_called = true;
            s.state = ModuleState::Disabled;
        }

        Ok(())
    }
}