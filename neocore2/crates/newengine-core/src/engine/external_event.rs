use super::Engine;

use crate::error::{EngineError, EngineResult, ModuleStage};
use crate::module::ModuleCtx;

use std::any::Any;
use std::panic::{self, AssertUnwindSafe};

impl<E: Send + 'static> Engine<E> {
    #[deprecated(
        note = "Use Engine::emit(...) + EventHub subscriptions instead of synchronous fan-out"
    )]
    pub fn dispatch_external_event(&mut self, event: &dyn Any) -> EngineResult<()> {
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
            let mut ctx = ModuleCtx::new(services, resources, bus, events, scheduler, exit_requested);

            let result = if self.catch_panics {
                #[allow(deprecated)]
                match panic::catch_unwind(AssertUnwindSafe(|| m.on_external_event(&mut ctx, event))) {
                    Ok(r) => r,
                    Err(payload) => {
                        *exit_requested = true;
                        Err(EngineError::Other(format!(
                            "panic in module callback (module='{module_id}' stage={:?} msg='{}')",
                            ModuleStage::ExternalEvent,
                            Self::panic_message(payload)
                        )))
                    }
                }
            } else {
                #[allow(deprecated)]
                m.on_external_event(&mut ctx, event)
            };

            result.map_err(|e| EngineError::with_module_stage(module_id, ModuleStage::ExternalEvent, e))?;

            if *exit_requested {
                shutdown.request();
                return Err(EngineError::ExitRequested);
            }
        }

        Ok(())
    }
}
