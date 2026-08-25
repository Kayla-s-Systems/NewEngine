use super::super::module_slot::ModuleState;
use super::super::{Engine, ModuleFaultTolerance};

use crate::engine::startup_graph;
use crate::error::{EngineError, EngineResult, ModuleStage};
use crate::lifecycle_events::EngineLifecycleEvent;
use crate::module::ModuleCtx;

impl<E: Send + 'static> Engine<E> {
    #[inline]
    pub(super) fn dispatch_startup_readiness_events(
        &mut self,
        origin: &'static str,
    ) -> EngineResult<()> {
        let plugin_count = self.plugins.snapshot().len();

        if self.plugins_loaded || self.engine_plugins_loaded {
            self.dispatch(EngineLifecycleEvent::EnginePluginsReady {
                loaded_count: plugin_count,
                origin,
            })?;
        }

        let module_count = self
            .modules
            .iter()
            .filter(|s| s.state == ModuleState::Running)
            .count();
        self.dispatch(EngineLifecycleEvent::EngineStartCompleted {
            module_count,
            plugin_count,
        })?;

        self.refresh_readiness_snapshot();
        newengine_ulog_api::ulog::debug!(
            "startup graph: after-startup-dispatch satisfied='{}' modules={}",
            self.startup_graph.satisfied_csv(),
            self.modules.len()
        );
        Ok(())
    }

    pub(crate) fn start_modules_ready_by_graph(
        &mut self,
        origin: &'static str,
    ) -> EngineResult<usize> {
        let mut activated_count = 0usize;

        for i in 0..self.modules.len() {
            self.sync_shutdown_state();
            if self.is_shutdown_requested() {
                return Err(EngineError::ExitRequested);
            }

            if self.modules[i].state != ModuleState::Pending {
                continue;
            }

            let module_id = self.modules[i].id();
            let requirements = self.modules[i].module.startup_requires();
            if !self.startup_graph.all_satisfied(requirements) {
                let missing = self
                    .startup_graph
                    .missing(requirements)
                    .map(|key| key.as_str())
                    .collect::<Vec<_>>()
                    .join(",");
                newengine_ulog_api::ulog::info!(
                    "startup graph: module gated module='{}' origin='{}' missing='{}'",
                    module_id,
                    origin,
                    if missing.is_empty() {
                        "-"
                    } else {
                        missing.as_str()
                    },
                );
                continue;
            }

            newengine_ulog_api::ulog::info!(
                "startup graph: starting module module='{}' origin='{}' requires='{}'",
                module_id,
                origin,
                startup_graph::readiness_csv(requirements),
            );

            let start_res = {
                let mut ctx = ModuleCtx::new(
                    self.services.as_ref(),
                    &mut self.resources,
                    &self.bus,
                    &self.events,
                    &mut self.scheduler,
                    self.shutdown.clone(),
                );
                newengine_plugin_host::with_host_module_callback(module_id, || {
                    self.modules[i].module.start(&mut ctx)
                })
            };

            if let Err(err) = start_res {
                match self.module_fault_tolerance {
                    ModuleFaultTolerance::Strict => {
                        self.request_exit()?;
                        return Err(EngineError::with_module_stage(
                            module_id,
                            ModuleStage::Start,
                            err,
                        ));
                    }
                    ModuleFaultTolerance::Resilient => {
                        let reason = format!("start failed: {err}");
                        newengine_ulog_api::ulog::error!(
                            "engine: module start failed: {} ({})",
                            module_id,
                            reason
                        );
                        self.modules[i].disable(reason);
                        self.shutdown_slot_by_index(i);
                        continue;
                    }
                }
            }

            self.modules[i].state = ModuleState::Running;
            activated_count += 1;
        }

        if activated_count > 0 {
            newengine_ulog_api::ulog::info!(
                "startup graph: activated modules origin='{}' count={} satisfied='{}'",
                origin,
                activated_count,
                self.startup_graph.satisfied_csv(),
            );
        }

        self.refresh_readiness_snapshot();
        if activated_count > 0 {
            self.log_startup_graph_snapshot(origin);
        } else {
            newengine_ulog_api::ulog::debug!(
                "startup graph: no modules activated origin='{}' satisfied='{}' modules={}",
                origin,
                self.startup_graph.satisfied_csv(),
                self.modules.len()
            );
        }
        Ok(activated_count)
    }
}
