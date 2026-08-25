#![forbid(unsafe_op_in_unsafe_fn)]

use super::module_slot::ModuleState;
use super::{Engine, EngineRunState, ModuleFaultTolerance};

use crate::error::{EngineError, EngineResult, ModuleStage};
use crate::module::{Module, ModuleCtx};

use std::panic::{self, AssertUnwindSafe};
use std::sync::OnceLock;
use std::time::Instant;

#[inline]
fn module_stage_profile_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        matches!(
            std::env::var("NEWENGINE_MODULE_STAGE_LOG").ok().as_deref(),
            Some("1") | Some("true") | Some("TRUE") | Some("on") | Some("ON")
        )
    })
}

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
        let stage_profile = matches!(stage, ModuleStage::Render)
            && (frame.frame_index <= 3 || frame.frame_index.is_multiple_of(30))
            && module_stage_profile_enabled();

        for s in self.modules.iter_mut() {
            if s.state != ModuleState::Running {
                continue;
            }

            if shutdown.is_requested() {
                return Err(EngineError::ExitRequested);
            }

            let module_id = s.id();
            let module_started = stage_profile.then(Instant::now);

            let result: EngineResult<()> = {
                let mut ctx = ModuleCtx::new(
                    services,
                    resources,
                    bus,
                    events,
                    scheduler,
                    shutdown.clone(),
                );
                ctx.set_frame(frame);

                if self.catch_panics {
                    match panic::catch_unwind(AssertUnwindSafe(|| {
                        newengine_plugin_host::with_host_module_callback(module_id, || {
                            call(s.module.as_mut(), &mut ctx)
                        })
                    })) {
                        Ok(r) => r,
                        Err(payload) => Err(EngineError::Other(format!(
                            "panic in module callback (module='{module_id}' stage={stage:?} msg='{}')",
                            Self::panic_message(payload)
                        ))),
                    }
                } else {
                    newengine_plugin_host::with_host_module_callback(module_id, || {
                        call(s.module.as_mut(), &mut ctx)
                    })
                }
            };

            if let Some(started) = module_started {
                newengine_ulog_api::ulog::info!(
                    "module.stage.profile: frame={} stage={:?} module='{}' elapsed_ms={:.3} ok={}",
                    frame.frame_index,
                    stage,
                    module_id,
                    started.elapsed().as_secs_f64() * 1000.0,
                    result.is_ok(),
                );
            }

            if let Err(e) = result {
                match self.module_fault_tolerance {
                    ModuleFaultTolerance::Strict => {
                        shutdown.request();
                        return Err(EngineError::with_module_stage(module_id, stage, e));
                    }
                    ModuleFaultTolerance::Resilient => {
                        let reason = format!("stage {stage:?} failed: {e}");
                        newengine_ulog_api::ulog::error!(
                            "engine: disabling module {} ({})",
                            module_id,
                            reason
                        );

                        s.disable(reason);

                        if !s.shutdown_called {
                            let mut ctx = ModuleCtx::new(
                                services,
                                resources,
                                bus,
                                events,
                                scheduler,
                                shutdown.clone(),
                            );
                            ctx.set_frame(frame);

                            let _ =
                                newengine_plugin_host::with_host_module_callback(module_id, || {
                                    s.module.shutdown(&mut ctx)
                                });
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
        self.activate_host_context();
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

            newengine_ulog_api::ulog::debug!(
                "engine shutdown: module shutdown begin id='{}'",
                module_id
            );
            crate::crash::record_breadcrumb(format!(
                "engine shutdown: module shutdown begin id={module_id}"
            ));
            let _ = newengine_plugin_host::with_host_module_callback(module_id, || {
                s.module.shutdown(&mut ctx)
            })
            .map_err(|e| EngineError::with_module_stage(module_id, ModuleStage::Shutdown, e));

            newengine_ulog_api::ulog::debug!(
                "engine shutdown: module shutdown completed id='{}'",
                module_id
            );
            crate::crash::record_breadcrumb(format!(
                "engine shutdown: module shutdown completed id={module_id}"
            ));
            s.shutdown_called = true;
            s.state = ModuleState::Disabled;
        }

        self.set_run_state(EngineRunState::ShutdownSystem);
        self.thread_pool.shutdown_and_join();
        self.plugins_shutdown();
        self.set_run_state(EngineRunState::Stopped);

        Ok(())
    }
}
