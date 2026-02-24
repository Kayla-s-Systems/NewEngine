use super::Engine;

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

        for m in self.modules.iter_mut() {
            if shutdown.is_requested() {
                *exit_requested = true;
            }
            if *exit_requested {
                shutdown.request();
                return Err(EngineError::ExitRequested);
            }

            let module_id = m.id();

            let mut ctx =
                ModuleCtx::new(services, resources, bus, events, scheduler, exit_requested);
            ctx.set_frame(frame);

            let result = if self.catch_panics {
                match panic::catch_unwind(AssertUnwindSafe(|| call(m.as_mut(), &mut ctx))) {
                    Ok(r) => r,
                    Err(payload) => {
                        *exit_requested = true;
                        Err(EngineError::Other(format!(
                            "panic in module callback (module='{module_id}' stage={stage:?} msg='{}')",
                            Self::panic_message(payload)
                        )))
                    }
                }
            } else {
                call(m.as_mut(), &mut ctx)
            };

            result.map_err(|e| EngineError::with_module_stage(module_id, stage, e))?;

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

        for m in self.modules.iter_mut().rev() {
            let module_id = m.id();

            let mut ctx = ModuleCtx::new(
                self.services.as_ref(),
                &mut self.resources,
                &self.bus,
                &self.events,
                &mut self.scheduler,
                &mut self.exit_requested,
            );

            let _ = m
                .shutdown(&mut ctx)
                .map_err(|e| EngineError::with_module_stage(module_id, ModuleStage::Shutdown, e));
        }

        Ok(())
    }
}
