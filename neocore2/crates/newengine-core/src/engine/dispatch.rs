#![forbid(unsafe_op_in_unsafe_fn)]

use super::module_slot::ModuleState;
use super::{Engine, ModuleFaultTolerance};

use crate::error::{EngineError, EngineResult, ModuleStage};
use crate::lifecycle_events::EngineLifecycleEvent;
use crate::module::ModuleCtx;

use std::any::Any;
use std::panic::{self, AssertUnwindSafe};

impl<E: Send + 'static> Engine<E> {
    /// Publish an event to `EventHub` and synchronously dispatch it to all
    /// currently running modules.
    ///
    /// Use this for lifecycle/readiness events that must be observed before the
    /// first frame is rendered. Regular high-frequency events should still use
    /// `emit()` plus typed `EventHub` subscriptions.
    pub fn dispatch<T>(&mut self, event: T) -> EngineResult<()>
    where
        T: Any + Clone + Send + Sync + 'static,
    {
        let event_type = std::any::type_name::<T>();
        log::debug!("dispatch: publish begin event_type='{}'", event_type);
        self.events.publish(event.clone())?;

        if let Some(lifecycle_event) = (&event as &dyn Any).downcast_ref::<EngineLifecycleEvent>() {
            log::info!(
                "dispatch: lifecycle event='{}' origin='{}' readiness_key='{}'",
                event_type,
                lifecycle_event.origin(),
                lifecycle_event.readiness_key(),
            );
            self.mark_readiness_observed(lifecycle_event);
            self.start_modules_ready_by_graph(lifecycle_event.readiness_key().as_str())?;
        }

        self.dispatch_to_running_modules(&event)
    }

    #[inline]
    pub(crate) fn dispatch_to_running_modules<T>(&mut self, event: &T) -> EngineResult<()>
    where
        T: Any + Send + Sync + 'static,
    {
        self.sync_shutdown_state();
        if self.is_exit_requested() {
            return Err(EngineError::ExitRequested);
        }

        let catch_panics = self.catch_panics;
        let module_fault_tolerance = self.module_fault_tolerance;

        let services = self.services.as_ref();
        let bus = &self.bus;
        let events = &self.events;
        let shutdown = &self.shutdown;

        let resources = &mut self.resources;
        let scheduler = &mut self.scheduler;
        let exit_requested = &mut self.exit_requested;
        let event_any = event as &dyn Any;
        let event_type = std::any::type_name::<T>();
        let mut delivered = 0usize;
        let mut skipped = 0usize;

        for s in self.modules.iter_mut() {
            if s.state != ModuleState::Running {
                skipped += 1;
                log::debug!(
                    "dispatch: skip module='{}' state='{:?}' event_type='{}'",
                    s.id(),
                    s.state,
                    event_type,
                );
                continue;
            }

            log::debug!(
                "dispatch: deliver module='{}' event_type='{}'",
                s.id(),
                event_type,
            );
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

                if catch_panics {
                    match panic::catch_unwind(AssertUnwindSafe(|| {
                        s.module.on_event(&mut ctx, event_any)
                    })) {
                        Ok(r) => r,
                        Err(payload) => Err(EngineError::Other(format!(
                            "panic in module callback (module='{module_id}' stage={:?} msg='{}')",
                            ModuleStage::ExternalEvent,
                            Self::panic_message(payload)
                        ))),
                    }
                } else {
                    s.module.on_event(&mut ctx, event_any)
                }
            };

            if let Err(e) = result {
                match module_fault_tolerance {
                    ModuleFaultTolerance::Strict => {
                        *exit_requested = true;
                        shutdown.request();
                        return Err(EngineError::with_module_stage(
                            module_id,
                            ModuleStage::ExternalEvent,
                            e,
                        ));
                    }
                    ModuleFaultTolerance::Resilient => {
                        let reason = format!("event dispatch failed: {e}");
                        log::error!("engine: disabling module {} ({})", module_id, reason);
                        s.disable(reason);

                        if !s.shutdown_called {
                            let mut ctx =
                                ModuleCtx::new(services, resources, bus, events, scheduler, exit_requested);
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

            delivered += 1;
        }

        log::debug!(
            "dispatch: complete event_type='{}' delivered={} skipped={}",
            event_type,
            delivered,
            skipped,
        );
        Ok(())
    }
}
