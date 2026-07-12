use super::super::{Engine, EngineRunState, ModuleFaultTolerance};
use super::planner;

use crate::error::EngineResult;
use crate::startup_status::{
    EngineIncrementalStartupState, EngineStartupPhase, EngineStartupSnapshot,
    EngineStartupStepOutcome, EngineStartupStepPhase,
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

        let phase = self
            .incremental_startup
            .as_ref()
            .map(|s| s.phase)
            .unwrap_or(EngineStartupStepPhase::Idle);
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
                self.set_incremental_phase(
                    if matches!(self.module_fault_tolerance, ModuleFaultTolerance::Strict) {
                        EngineStartupStepPhase::ValidateApiContracts
                    } else {
                        EngineStartupStepPhase::EnterGameInit
                    },
                );
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
                            planner::reorder_slots_by_order(
                                std::mem::take(&mut self.modules),
                                &order,
                            )
                        })
                    }
                    ModuleFaultTolerance::Resilient => {
                        let plan = planner::build_resilient_module_plan(&self.modules);
                        Ok(planner::partition_slots_by_resilient_plan(
                            std::mem::take(&mut self.modules),
                            plan,
                        ))
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
                    format!(
                        "{} plugin descriptor(s) are visible to the host.",
                        self.plugins.snapshot().len()
                    ),
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
}
