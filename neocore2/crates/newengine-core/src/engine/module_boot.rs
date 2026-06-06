mod planner;

use super::module_slot::ModuleState;
use super::{Engine, EngineRunState, ModuleFaultTolerance};

use crate::error::{EngineError, EngineResult, ModuleStage};
use crate::lifecycle_events::EngineLifecycleEvent;
use crate::module::ModuleCtx;
use crate::startup_status::{
    EngineIncrementalStartupState, EngineStartupPhase, EngineStartupSnapshot,
    EngineStartupStepOutcome, EngineStartupStepPhase, EngineStartupSystemPhase,
    EngineStartupSystemStatus,
};


impl<E: Send + 'static> Engine<E> {
    /// Compatibility entry point: drives the real incremental startup pump to
    /// completion in the current thread. Platform hosts should prefer
    /// `start_incremental_step()` so the engine.ui loading projection can be published
    /// between expensive systems/modules.
    pub fn start(&mut self) -> EngineResult<()> {
        loop {
            let outcome = self.start_incremental_step()?;
            if outcome.finished {
                return Ok(());
            }
        }
    }

    /// Advances engine startup by one deterministic FSM/pipeline step.
    ///
    /// This is the source for real loading-screen status: the snapshot contains
    /// the core FSM state, current startup phase, current module, plugin count
    /// and exact failure context.
    pub fn start_incremental_step(&mut self) -> EngineResult<EngineStartupStepOutcome> {
        if self.run_state().is_running() {
            let snapshot = EngineStartupSnapshot::complete(
                self.run_state().as_str(),
                self.modules.len(),
                self.plugins.snapshot().len(),
            );
            self.publish_startup_snapshot(snapshot.clone());
            return Ok(EngineStartupStepOutcome::complete(snapshot));
        }

        if self.incremental_startup.is_none() {
            self.incremental_startup = Some(EngineIncrementalStartupState {
                phase: EngineStartupStepPhase::EnterSystemInit,
                ..EngineIncrementalStartupState::default()
            });
            let snapshot = self.make_startup_snapshot(
                EngineStartupPhase::SystemInit,
                "Initializing core systems...",
                "Core FSM is entering init-system and preparing startup services.",
                0.02,
                None,
                0,
                self.modules.len(),
            );
            self.publish_startup_snapshot(snapshot.clone());
            return Ok(EngineStartupStepOutcome::running(snapshot));
        }

        let phase = self.incremental_startup.as_ref().map(|s| s.phase).unwrap_or(EngineStartupStepPhase::Idle);
        match phase {
            EngineStartupStepPhase::Idle => {
                self.set_incremental_phase(EngineStartupStepPhase::EnterSystemInit);
                let snapshot = self.make_startup_snapshot(
                    EngineStartupPhase::SystemInit,
                    "Initializing core systems...",
                    "Core FSM is entering init-system and preparing startup services.",
                    0.02,
                    None,
                    0,
                    self.modules.len(),
                );
                self.publish_startup_snapshot(snapshot.clone());
                Ok(EngineStartupStepOutcome::running(snapshot))
            }
            EngineStartupStepPhase::EnterSystemInit => {
                if let Err(e) = self.enter_system_init() {
                    return self.fail_incremental_startup(
                        EngineStartupPhase::SystemInit,
                        "Core system initialization failed.",
                        "The FSM could not enter init-system cleanly.",
                        0.04,
                        None,
                        e,
                    );
                }
                self.set_incremental_phase(if matches!(self.module_fault_tolerance, ModuleFaultTolerance::Strict) {
                    EngineStartupStepPhase::ValidateApiContracts
                } else {
                    EngineStartupStepPhase::EnterGameInit
                });
                let snapshot = self.make_startup_snapshot(
                    EngineStartupPhase::SystemInit,
                    "Core FSM entered init-system.",
                    "Shutdown token, scheduler, resources and service registries are ready for module bootstrap.",
                    0.08,
                    None,
                    0,
                    self.modules.len(),
                );
                self.publish_startup_snapshot(snapshot.clone());
                Ok(EngineStartupStepOutcome::running(snapshot))
            }
            EngineStartupStepPhase::ValidateApiContracts => {
                if let Err(e) = self.validate_api_contracts_strict() {
                    return self.fail_incremental_startup(
                        EngineStartupPhase::ApiContracts,
                        "API contract validation failed.",
                        "A module requires a service/API version that the host cannot provide.",
                        0.13,
                        None,
                        e,
                    );
                }
                self.set_incremental_phase(EngineStartupStepPhase::EnterGameInit);
                let snapshot = self.make_startup_snapshot(
                    EngineStartupPhase::ApiContracts,
                    "API contracts validated.",
                    "Strict API/service requirements passed before module initialization.",
                    0.16,
                    None,
                    0,
                    self.modules.len(),
                );
                self.publish_startup_snapshot(snapshot.clone());
                Ok(EngineStartupStepOutcome::running(snapshot))
            }
            EngineStartupStepPhase::EnterGameInit => {
                self.enter_game_init();
                self.set_incremental_phase(EngineStartupStepPhase::PrepareModuleOrder);
                let snapshot = self.make_startup_snapshot(
                    EngineStartupPhase::GameInit,
                    "Core FSM entered init-game.",
                    "Runtime modules can now be ordered and initialized through the startup graph.",
                    0.22,
                    None,
                    0,
                    self.modules.len(),
                );
                self.publish_startup_snapshot(snapshot.clone());
                Ok(EngineStartupStepOutcome::running(snapshot))
            }
            EngineStartupStepPhase::PrepareModuleOrder => {
                let result = match self.module_fault_tolerance {
                    ModuleFaultTolerance::Strict => {
                        planner::build_strict_module_order(&self.modules).map(|order| {
                            planner::reorder_slots_by_order(std::mem::take(&mut self.modules), &order)
                        })
                    }
                    ModuleFaultTolerance::Resilient => {
                        let plan = planner::build_resilient_module_plan(&self.modules);
                        Ok(planner::partition_slots_by_resilient_plan(std::mem::take(&mut self.modules), plan))
                    }
                };

                match result {
                    Ok(slots) => {
                        self.modules = slots;
                        if let Some(state) = &mut self.incremental_startup {
                            state.phase = EngineStartupStepPhase::InitModules;
                            state.index = 0;
                            state.initialized = 0;
                            state.module_total = self.modules.len();
                        }
                        let snapshot = self.make_startup_snapshot(
                            EngineStartupPhase::ModuleOrder,
                            "Startup module order resolved.",
                            format!(
                                "{} module(s) scheduled with {:?} fault tolerance.",
                                self.modules.len(),
                                self.module_fault_tolerance
                            ),
                            0.28,
                            None,
                            0,
                            self.modules.len(),
                        );
                        self.publish_startup_snapshot(snapshot.clone());
                        Ok(EngineStartupStepOutcome::running(snapshot))
                    }
                    Err(e) => self.fail_incremental_startup(
                        EngineStartupPhase::ModuleOrder,
                        "Module order resolution failed.",
                        "The dependency planner could not produce a safe startup order.",
                        0.26,
                        None,
                        e,
                    ),
                }
            }
            EngineStartupStepPhase::InitModules => self.start_incremental_init_module(),
            EngineStartupStepPhase::StartupGraphInitial => {
                self.refresh_readiness_snapshot();
                self.log_startup_graph_snapshot("after-init");
                if let Err(e) = self.start_modules_ready_by_graph("initial") {
                    return self.fail_incremental_startup(
                        EngineStartupPhase::StartupGraph,
                        "Startup graph activation failed.",
                        "A module failed while processing startup graph readiness gates.",
                        0.70,
                        None,
                        e,
                    );
                }
                self.set_incremental_phase(EngineStartupStepPhase::LoadRuntimePlugins);
                let snapshot = self.make_startup_snapshot(
                    EngineStartupPhase::StartupGraph,
                    "Startup graph evaluated.",
                    "Modules whose readiness requirements are satisfied have been activated.",
                    0.74,
                    None,
                    self.modules.len(),
                    self.modules.len(),
                );
                self.publish_startup_snapshot(snapshot.clone());
                Ok(EngineStartupStepOutcome::running(snapshot))
            }
            EngineStartupStepPhase::LoadRuntimePlugins => {
                if !self.plugins_loaded && !self.engine_plugins_loaded {
                    if let Err(e) = self.try_load_plugins_once() {
                        return self.fail_incremental_startup(
                            EngineStartupPhase::RuntimePlugins,
                            "Runtime plugin load failed.",
                            "Plugin discovery or dynamic loading failed before plugin start.",
                            0.79,
                            None,
                            e,
                        );
                    }
                }
                self.set_incremental_phase(EngineStartupStepPhase::StartPlugins);
                let snapshot = self.make_startup_snapshot(
                    EngineStartupPhase::RuntimePlugins,
                    "Runtime plugins loaded.",
                    format!("{} plugin descriptor(s) are visible to the host.", self.plugins.snapshot().len()),
                    0.82,
                    None,
                    self.modules.len(),
                    self.modules.len(),
                );
                self.publish_startup_snapshot(snapshot.clone());
                Ok(EngineStartupStepOutcome::running(snapshot))
            }
            EngineStartupStepPhase::StartPlugins => {
                // Engine plugin diagnostics are already emitted immediately after
                // engine plugin discovery. Do not print the same descriptor table
                // again during the later startup-FSM step unless plugins were
                // actually loaded by this phase.
                if !self.engine_plugins_loaded {
                    self.log_plugins_diagnostics("after runtime plugin load");
                }
                if let Err(e) = self.plugins_start_all() {
                    return self.fail_incremental_startup(
                        EngineStartupPhase::PluginStart,
                        "Plugin startup failed.",
                        "One or more plugin-owned services failed to enter running state.",
                        0.86,
                        None,
                        e,
                    );
                }
                self.set_incremental_phase(EngineStartupStepPhase::ValidateRuntimeServiceContracts);
                let snapshot = self.make_startup_snapshot(
                    EngineStartupPhase::PluginStart,
                    "Plugin services started.",
                    "Plugin-owned services have completed their startup hooks.",
                    0.88,
                    None,
                    self.modules.len(),
                    self.modules.len(),
                );
                self.publish_startup_snapshot(snapshot.clone());
                Ok(EngineStartupStepOutcome::running(snapshot))
            }
            EngineStartupStepPhase::ValidateRuntimeServiceContracts => {
                let plugins = self.plugins.snapshot();
                if let Err(e) =
                    crate::startup::api_contracts::validate_runtime_service_contracts(&plugins)
                {
                    return self.fail_incremental_startup(
                        EngineStartupPhase::ServiceContracts,
                        "Runtime service contract validation failed.",
                        "A required runtime service is missing or exposes an incompatible method set.",
                        0.90,
                        None,
                        e,
                    );
                }
                self.set_incremental_phase(EngineStartupStepPhase::DispatchReadiness);
                let snapshot = self.make_startup_snapshot(
                    EngineStartupPhase::ServiceContracts,
                    "Runtime service contracts validated.",
                    "AssetManager, renderer, physics and platform service contracts match the expected ABI surface.",
                    0.91,
                    None,
                    self.modules.len(),
                    self.modules.len(),
                );
                self.publish_startup_snapshot(snapshot.clone());
                Ok(EngineStartupStepOutcome::running(snapshot))
            }
            EngineStartupStepPhase::DispatchReadiness => {
                if let Err(e) = self.dispatch_startup_readiness_events("engine.start.incremental") {
                    return self.fail_incremental_startup(
                        EngineStartupPhase::ReadinessEvents,
                        "Readiness event dispatch failed.",
                        "Startup readiness events could not be delivered to running modules.",
                        0.92,
                        None,
                        e,
                    );
                }
                self.set_incremental_phase(EngineStartupStepPhase::EnterRunning);
                let snapshot = self.make_startup_snapshot(
                    EngineStartupPhase::ReadinessEvents,
                    "Readiness events dispatched.",
                    "EnginePluginsReady and EngineStartCompleted readiness gates have been published.",
                    0.94,
                    None,
                    self.modules.len(),
                    self.modules.len(),
                );
                self.publish_startup_snapshot(snapshot.clone());
                Ok(EngineStartupStepOutcome::running(snapshot))
            }
            EngineStartupStepPhase::EnterRunning => {
                self.set_run_state(EngineRunState::Running);
                self.set_incremental_phase(EngineStartupStepPhase::Complete);
                let snapshot = EngineStartupSnapshot::complete(
                    self.run_state().as_str(),
                    self.modules.len(),
                    self.plugins.snapshot().len(),
                );
                self.publish_startup_snapshot(snapshot.clone());
                Ok(EngineStartupStepOutcome::complete(snapshot))
            }
            EngineStartupStepPhase::Complete => {
                let snapshot = EngineStartupSnapshot::complete(
                    self.run_state().as_str(),
                    self.modules.len(),
                    self.plugins.snapshot().len(),
                );
                self.publish_startup_snapshot(snapshot.clone());
                Ok(EngineStartupStepOutcome::complete(snapshot))
            }
        }
    }

    fn start_incremental_init_module(&mut self) -> EngineResult<EngineStartupStepOutcome> {
        let total = self.modules.len();
        let mut index = self.incremental_startup.as_ref().map(|s| s.index).unwrap_or(0);

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
                Err(err) => match self.module_fault_tolerance {
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
                },
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

    #[inline]
    fn module_init_progress(&self, done: usize, total: usize) -> f32 {
        if total == 0 {
            0.66
        } else {
            0.32 + (done as f32 / total as f32).clamp(0.0, 1.0) * 0.34
        }
    }

    fn make_startup_snapshot(
        &self,
        phase: EngineStartupPhase,
        status: impl Into<String>,
        detail: impl Into<String>,
        progress_01: f32,
        current_module: Option<String>,
        module_index: usize,
        module_total: usize,
    ) -> EngineStartupSnapshot {
        let plugin_count = self.plugins.snapshot().len();
        let systems = self.make_startup_systems(
            phase,
            progress_01,
            current_module.as_deref(),
            module_index,
            module_total,
            plugin_count,
        );

        EngineStartupSnapshot::running(
            phase,
            self.run_state().as_str(),
            status,
            detail,
            progress_01,
            current_module,
            module_index,
            module_total,
            plugin_count,
            systems,
        )
    }

    fn make_startup_systems(
        &self,
        phase: EngineStartupPhase,
        progress_01: f32,
        current_module: Option<&str>,
        module_index: usize,
        module_total: usize,
        plugin_count: usize,
    ) -> Vec<EngineStartupSystemStatus> {
        let modules_phase = startup_system_phase(
            phase == EngineStartupPhase::ModuleInit,
            progress_01,
            0.70,
        );
        let plugins_phase = startup_system_phase(
            matches!(phase, EngineStartupPhase::RuntimePlugins | EngineStartupPhase::PluginStart),
            progress_01,
            0.88,
        );
        let contracts_phase = startup_system_phase(
            phase == EngineStartupPhase::ServiceContracts,
            progress_01,
            0.91,
        );
        let readiness_phase = startup_system_phase(
            phase == EngineStartupPhase::ReadinessEvents,
            progress_01,
            0.94,
        );

        vec![
            EngineStartupSystemStatus::new(
                "fsm",
                "CORE FSM",
                if self.run_state().is_booting() {
                    EngineStartupSystemPhase::Running
                } else {
                    EngineStartupSystemPhase::Ready
                },
                self.run_state().as_str().to_ascii_uppercase(),
                format!("Core lifecycle state is '{}'.", self.run_state().as_str()),
                Some(progress_01),
            ),
            EngineStartupSystemStatus::new(
                "modules",
                "MODULES",
                modules_phase,
                startup_system_state_label(modules_phase, "INIT"),
                current_module
                    .map(|m| format!("Processing module '{m}' ({module_index}/{module_total})."))
                    .unwrap_or_else(|| format!("{} module slot(s) registered.", self.modules.len())),
                (module_total > 0).then_some(
                    (module_index as f32 / module_total as f32).clamp(0.0, 1.0),
                ),
            ),
            EngineStartupSystemStatus::new(
                "plugins",
                "PLUGINS",
                plugins_phase,
                startup_system_state_label(plugins_phase, "LOAD"),
                format!("{plugin_count} plugin descriptor(s) known to the host."),
                Some(progress_until_ready(progress_01, 0.88)),
            ),
            EngineStartupSystemStatus::new(
                "contracts",
                "CONTRACTS",
                contracts_phase,
                startup_system_state_label(contracts_phase, "CHECK"),
                "Runtime service contracts are validated before readiness events reach gameplay modules.",
                Some(progress_until_ready(progress_01, 0.91)),
            ),
            EngineStartupSystemStatus::new(
                "readiness",
                "READINESS",
                readiness_phase,
                startup_system_state_label(readiness_phase, "EVENTS"),
                "Startup graph readiness facts are being collected and dispatched.",
                Some(progress_until_ready(progress_01, 0.94)),
            ),
            EngineStartupSystemStatus::new(
                "diagnostics",
                "DIAGNOSTICS",
                EngineStartupSystemPhase::Running,
                phase.human_label(),
                format!(
                    "phase='{}' run_state='{}'",
                    phase.as_str(),
                    self.run_state().as_str()
                ),
                Some(progress_01),
            ),
        ]
    }

    fn publish_startup_snapshot(&mut self, snapshot: EngineStartupSnapshot) {
        self.resources.insert(snapshot.clone());
        let _ = self.events.publish(snapshot.clone());
        self.startup_snapshot = snapshot;
    }

    fn fail_incremental_startup(
        &mut self,
        phase: EngineStartupPhase,
        status: impl Into<String>,
        detail: impl Into<String>,
        progress_01: f32,
        current_module: Option<String>,
        err: EngineError,
    ) -> EngineResult<EngineStartupStepOutcome> {
        if !self.run_state().is_terminal() {
            self.set_run_state(EngineRunState::Faulted);
        }
        let error = err.to_string();
        let current_module_for_log = current_module.as_deref().unwrap_or("-");
        newengine_ulog_api::ulog::error!(
            "startup fsm: failed phase='{}' run_state='{}' module='{}' progress={:.2} err='{}'",
            phase.as_str(),
            self.run_state().as_str(),
            current_module_for_log,
            progress_01,
            error
        );
        let module_index = self.incremental_startup.as_ref().map(|s| s.index).unwrap_or(0);
        let module_total = self.modules.len();
        let snapshot = EngineStartupSnapshot::failed(
            phase,
            self.run_state().as_str(),
            status,
            detail,
            progress_01,
            current_module,
            module_index,
            module_total,
            self.plugins.snapshot().len(),
            error,
        );
        self.publish_startup_snapshot(snapshot);
        Err(err)
    }

    #[inline]
    fn set_incremental_phase(&mut self, phase: EngineStartupStepPhase) {
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

    fn shutdown_initialized_modules(&mut self, initialized: usize) {
        let end = initialized.min(self.modules.len());
        for i in (0..end).rev() {
            self.shutdown_slot_by_index(i);
        }
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
                    if missing.is_empty() { "-" } else { missing.as_str() },
                );
                continue;
            }

            newengine_ulog_api::ulog::info!(
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
                        newengine_ulog_api::ulog::error!("engine: module start failed: {} ({})", module_id, reason);
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

}
#[inline]
fn startup_system_phase(
    running: bool,
    progress_01: f32,
    ready_at: f32,
) -> EngineStartupSystemPhase {
    if running {
        EngineStartupSystemPhase::Running
    } else if progress_01 >= ready_at {
        EngineStartupSystemPhase::Ready
    } else {
        EngineStartupSystemPhase::Waiting
    }
}

#[inline]
fn startup_system_state_label(
    phase: EngineStartupSystemPhase,
    running_label: &'static str,
) -> &'static str {
    match phase {
        EngineStartupSystemPhase::Waiting => "WAIT",
        EngineStartupSystemPhase::Running => running_label,
        EngineStartupSystemPhase::Ready => "READY",
        EngineStartupSystemPhase::Degraded => "DEGRADED",
        EngineStartupSystemPhase::Failed => "ERR",
    }
}

#[inline]
fn progress_until_ready(progress_01: f32, ready_at: f32) -> f32 {
    if progress_01 >= ready_at {
        1.0
    } else {
        progress_01.clamp(0.0, 1.0)
    }
}
