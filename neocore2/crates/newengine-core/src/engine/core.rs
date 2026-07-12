use crate::error::{EngineError, EngineResult};
use crate::events::EventHub;
use crate::module::{Bus, Module, Resources, Services};
use crate::sched::Scheduler;
use crate::startup_status::{EngineIncrementalStartupState, EngineStartupSnapshot};
use crate::sync::ShutdownToken;
use crate::threading::{ThreadPoolHandle, ThreadPoolManager, ThreadPoolSnapshot};
use newengine_plugin_host::{
    init_host_context, init_plugin_config_service, PluginControlQueue, PluginManager,
};

use newengine_math::{register_engine_builtins, MathRegistry};

use newengine_math::collections_prelude::NeHashSet as HashSet;
use std::any::Any;
use std::path::PathBuf;
use std::time::Instant;

use super::module_slot::ModuleSlot;
use super::run_state::{EngineFsm, EngineRunState};
use super::startup_graph::StartupReadinessGraph;
use super::{EngineConfig, ModuleFaultTolerance, PluginFaultTolerance};

pub struct Engine<E: Send + 'static> {
    pub(super) fixed_dt: f32,
    pub(super) catch_panics: bool,
    pub(super) services: Box<dyn Services>,
    pub(super) modules: Vec<ModuleSlot<E>>,
    pub(super) module_fault_tolerance: ModuleFaultTolerance,
    pub(super) plugin_fault_tolerance: PluginFaultTolerance,
    pub(super) module_ids: HashSet<&'static str>,

    pub resources: Resources,
    pub(super) bus: Bus<E>,

    pub(super) events: EventHub,
    pub(super) scheduler: Scheduler,
    pub(super) thread_pool: ThreadPoolManager,
    pub(super) startup_graph: StartupReadinessGraph,
    pub(super) startup_snapshot: EngineStartupSnapshot,
    pub(super) incremental_startup: Option<EngineIncrementalStartupState>,

    pub(super) plugins: PluginManager,
    pub(super) plugins_loaded: bool,
    pub(super) engine_plugins_loaded: bool,
    pub(super) plugins_dir: Option<PathBuf>,
    pub(super) plugin_discovery_scan: Option<super::plugin_discovery::PluginDiscoveryScanTask>,

    pub(super) shutdown: ShutdownToken,
    pub(super) fsm: EngineFsm,

    pub(super) frame_index: u64,
    pub(super) fixed_tick: u64,
    pub(super) last: Instant,
    pub(super) acc: f32,
}

impl<E: Send + 'static> Engine<E> {
    #[inline]
    pub fn request_exit(&mut self) -> EngineResult<()> {
        let transition = self.fsm.request_shutdown();
        self.log_fsm_transition(transition);
        self.shutdown.request();
        Ok(())
    }

    #[inline]
    pub fn run_state(&self) -> EngineRunState {
        self.fsm.state()
    }

    #[inline]
    pub fn startup_status(&self) -> EngineStartupSnapshot {
        self.startup_snapshot.clone()
    }

    #[inline]
    pub(super) fn set_run_state(&mut self, next: EngineRunState) {
        let transition = self.fsm.transition(next);
        self.log_fsm_transition(transition);
        self.propagate_shutdown_request();
    }

    #[inline]
    pub(super) fn log_fsm_transition(&self, transition: super::run_state::EngineFsmTransition) {
        if !transition.changed {
            return;
        }
        if transition.valid {
            newengine_ulog_api::ulog::info!(
                "engine state: {} -> {}",
                transition.previous.as_str(),
                transition.next.as_str()
            );
        } else {
            newengine_ulog_api::ulog::error!(
                "engine state: invalid transition {} -> {}; forced {}",
                transition.previous.as_str(),
                transition.next.as_str(),
                EngineRunState::Faulted.as_str()
            );
        }
    }

    #[inline]
    pub fn shutdown_token(&self) -> ShutdownToken {
        self.shutdown.clone()
    }

    #[inline]
    pub fn events(&self) -> &EventHub {
        &self.events
    }

    #[inline]
    pub fn thread_pool(&self) -> ThreadPoolHandle {
        self.thread_pool.handle()
    }

    #[inline]
    pub fn thread_pool_snapshot(&self) -> ThreadPoolSnapshot {
        self.thread_pool.snapshot()
    }

    pub fn emit<T>(&self, event: T) -> EngineResult<()>
    where
        T: Any + Send + 'static + Sync,
    {
        self.events.publish(event)
    }

    pub fn new(
        fixed_dt_ms: u32,
        services: Box<dyn Services>,
        bus: Bus<E>,
        shutdown: ShutdownToken,
    ) -> EngineResult<Self> {
        let config = if let Some(startup) = crate::startup::last_startup_config() {
            EngineConfig::new(fixed_dt_ms)
                .with_plugins_dir(Some(startup.modules_dir.clone()))
                .with_plugin_overrides(startup.plugins.clone())
        } else {
            EngineConfig::new(fixed_dt_ms)
        };
        Self::new_with_config(config, services, bus, shutdown)
    }

    pub fn new_with_config(
        mut config: EngineConfig,
        services: Box<dyn Services>,
        bus: Bus<E>,
        shutdown: ShutdownToken,
    ) -> EngineResult<Self> {
        if let Some(startup) = crate::startup::last_startup_config() {
            if config.plugins_dir.is_none() {
                config.plugins_dir = Some(startup.modules_dir.clone());
            }

            if config.plugin_overrides.is_empty() && !startup.plugins.is_empty() {
                config.plugin_overrides = startup.plugins.clone();
            }
        }

        let fixed_dt = (config.fixed_dt_ms as f32 / 1000.0).max(0.001);

        let mut resources = Resources::default();
        resources.insert(PluginControlQueue::default());

        let events = EventHub::new();
        let thread_pool = ThreadPoolManager::new_with_event_hub(config.thread_pool, events.clone());
        resources.insert(thread_pool.handle());

        init_host_context();
        init_plugin_config_service(config.plugin_overrides.clone());

        register_engine_builtins(MathRegistry::global())
            .map_err(|e| EngineError::Other(format!("math init failed: {e}")))?;

        Ok(Self {
            fixed_dt,
            catch_panics: config.catch_panics,
            services,
            modules: Vec::new(),
            module_fault_tolerance: config.module_fault_tolerance,
            plugin_fault_tolerance: config.plugin_fault_tolerance,
            module_ids: HashSet::default(),

            resources,
            bus,
            events,
            scheduler: Scheduler::new(),
            thread_pool,
            startup_graph: StartupReadinessGraph::default(),
            startup_snapshot: EngineStartupSnapshot::idle(EngineRunState::Created.as_str()),
            incremental_startup: None,

            plugins: PluginManager::new(),
            plugins_loaded: false,
            engine_plugins_loaded: false,
            plugins_dir: config.plugins_dir,
            plugin_discovery_scan: None,

            shutdown,
            fsm: EngineFsm::new(),

            frame_index: 0,
            fixed_tick: 0,
            last: Instant::now(),
            acc: 0.0,
        })
    }

    #[inline]
    pub fn resources_mut(&mut self) -> &mut Resources {
        &mut self.resources
    }

    #[inline]
    pub fn bus(&self) -> &Bus<E> {
        &self.bus
    }

    pub fn register_module(&mut self, module: Box<dyn Module<E>>) -> EngineResult<()> {
        self.sync_shutdown_state();

        let id = module.id();
        if self.module_ids.contains(id) {
            return Err(EngineError::Other(format!(
                "module already registered: {id}"
            )));
        }

        self.modules.push(ModuleSlot::new(module));
        self.module_ids.insert(id);
        Ok(())
    }

    #[inline]
    pub(super) fn is_shutdown_requested(&self) -> bool {
        self.fsm.is_shutdown_requested() || self.shutdown.is_requested()
    }

    #[inline]
    pub(super) fn sync_shutdown_state(&mut self) {
        let transition = self
            .fsm
            .sync_external_shutdown(self.shutdown.is_requested());
        self.log_fsm_transition(transition);
    }

    #[inline]
    pub(super) fn propagate_shutdown_request(&mut self) {
        if self.fsm.is_shutdown_requested() {
            self.shutdown.request();
        }
    }
}
