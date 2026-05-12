mod planner;

use super::module_slot::{ModuleSlot, ModuleState};
use super::{Engine, EngineRunState, ModuleFaultTolerance};

use crate::error::{EngineError, EngineResult, ModuleStage};
use crate::lifecycle_events::EngineLifecycleEvent;
use crate::module::ModuleCtx;


impl<E: Send + 'static> Engine<E> {
    pub fn start(&mut self) -> EngineResult<()> {
        match self.module_fault_tolerance {
            ModuleFaultTolerance::Strict => self.start_strict(),
            ModuleFaultTolerance::Resilient => self.start_resilient(),
        }
    }

    fn start_strict(&mut self) -> EngineResult<()> {
        self.enter_system_init()?;
        self.validate_api_contracts_strict()?;
        self.enter_game_init();

        let order = planner::build_strict_module_order(&self.modules)?;
        let mut sorted = planner::reorder_slots_by_order(std::mem::take(&mut self.modules), &order);

        #[inline]
        fn shutdown_modules<E: Send + 'static>(engine: &mut Engine<E>, slots: &mut [ModuleSlot<E>]) {
            for s in slots.iter_mut().rev() {
                if s.shutdown_called {
                    continue;
                }
                let mut ctx = ModuleCtx::new(
                    engine.services.as_ref(),
                    &mut engine.resources,
                    &engine.bus,
                    &engine.events,
                    &mut engine.scheduler,
                    engine.shutdown.clone(),
                );
                let _ = s.module.shutdown(&mut ctx);
                s.shutdown_called = true;
                s.state = ModuleState::Disabled;
            }
        }

        let mut initialized = 0usize;

        for i in 0..sorted.len() {
            self.sync_shutdown_state();

            let init_result = {
                let s = &mut sorted[i];
                let mut ctx = ModuleCtx::new(
                    self.services.as_ref(),
                    &mut self.resources,
                    &self.bus,
                    &self.events,
                    &mut self.scheduler,
                    self.shutdown.clone(),
                );
                s.module.init(&mut ctx)
            };

            if let Err(err) = init_result {
                shutdown_modules(self, &mut sorted[..initialized]);
                return Err(EngineError::with_module_stage(
                    sorted[i].id(),
                    ModuleStage::Init,
                    err,
                ));
            }

            initialized += 1;

            self.sync_shutdown_state();
            if self.is_shutdown_requested() {
                shutdown_modules(self, &mut sorted[..initialized]);
                return Err(EngineError::ExitRequested);
            }
        }

        self.modules = sorted;
        self.complete_startup("engine.start.strict")
    }

    fn start_resilient(&mut self) -> EngineResult<()> {
        self.enter_system_init()?;
        self.enter_game_init();

        let plan = planner::build_resilient_module_plan(&self.modules);
        let mut new_slots = planner::partition_slots_by_resilient_plan(
            std::mem::take(&mut self.modules),
            plan,
        );

        for i in 0..new_slots.len() {
            self.sync_shutdown_state();
            if self.is_shutdown_requested() {
                break;
            }

            if new_slots[i].state != ModuleState::Pending {
                continue;
            }

            let module_id = new_slots[i].id();

            let init_res = {
                let mut ctx = ModuleCtx::new(
                    self.services.as_ref(),
                    &mut self.resources,
                    &self.bus,
                    &self.events,
                    &mut self.scheduler,
                    self.shutdown.clone(),
                );
                new_slots[i].module.init(&mut ctx)
            };

            if let Err(err) = init_res {
                let reason = format!("init failed: {err}");
                log::error!("engine: module init failed: {} ({})", module_id, reason);
                new_slots[i].disable(reason);
                self.shutdown_slot(&mut new_slots[i]);
                continue;
            }
        }

        self.modules = new_slots;
        self.complete_startup("engine.start.resilient")
    }

    #[inline]
    fn enter_system_init(&mut self) -> EngineResult<()> {
        self.set_run_state(EngineRunState::InitSystem);
        self.last = std::time::Instant::now();
        self.sync_shutdown_state();

        if self.is_shutdown_requested() {
            return Err(EngineError::ExitRequested);
        }

        Ok(())
    }

    #[inline]
    fn enter_game_init(&mut self) {
        self.set_run_state(EngineRunState::InitGame);
    }

    fn complete_startup(&mut self, origin: &'static str) -> EngineResult<()> {
        self.refresh_readiness_snapshot();
        self.log_startup_graph_snapshot("after-init");
        self.start_modules_ready_by_graph("initial")?;

        if !self.plugins_loaded && !self.engine_plugins_loaded {
            self.try_load_plugins_once()?;
        }
        self.log_plugins_diagnostics("after module init");
        self.plugins_start_all()?;
        self.dispatch_startup_readiness_events(origin)?;
        self.set_run_state(EngineRunState::Running);

        Ok(())
    }

    #[inline]
    fn dispatch_startup_readiness_events(&mut self, origin: &'static str) -> EngineResult<()> {
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
        self.log_startup_graph_snapshot("after-startup-dispatch");
        Ok(())
    }

    pub(crate) fn start_modules_ready_by_graph(&mut self, origin: &'static str) -> EngineResult<usize> {
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
                log::info!(
                    "startup graph: module gated module='{}' origin='{}' missing='{}'",
                    module_id,
                    origin,
                    if missing.is_empty() { "-" } else { missing.as_str() },
                );
                continue;
            }

            log::info!(
                "startup graph: starting module module='{}' origin='{}' requires='{}'",
                module_id,
                origin,
                super::startup_graph::readiness_csv(requirements),
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
                self.modules[i].module.start(&mut ctx)
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
                        log::error!("engine: module start failed: {} ({})", module_id, reason);
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
            log::info!(
                "startup graph: activated modules origin='{}' count={} satisfied='{}'",
                origin,
                activated_count,
                self.startup_graph.satisfied_csv(),
            );
        }

        self.refresh_readiness_snapshot();
        self.log_startup_graph_snapshot(origin);
        Ok(activated_count)
    }

    #[inline]
    fn shutdown_slot_by_index(&mut self, index: usize) {
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

    #[inline]
    fn shutdown_slot(&mut self, s: &mut ModuleSlot<E>) {
        if s.shutdown_called {
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
        let _ = s.module.shutdown(&mut ctx);
        s.shutdown_called = true;
        s.state = ModuleState::Disabled;
    }
}