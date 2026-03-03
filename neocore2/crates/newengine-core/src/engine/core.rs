use crate::error::{EngineError, EngineResult};
use crate::events::EventHub;
use crate::module::{Bus, Module, Resources, Services};
use crate::plugins::plugin_config_service::init_plugin_config_service;
use crate::plugins::{init_host_context, PluginControlQueue, PluginManager};
use crate::sched::Scheduler;
use crate::sync::ShutdownToken;

use newengine_math::{register_engine_builtins, MathRegistry};

use std::any::Any;
use std::collections::HashSet;
use std::path::PathBuf;
use std::time::Instant;

use super::module_slot::ModuleSlot;
use super::{EngineConfig, ModuleFaultTolerance};

pub struct Engine<E: Send + 'static> {
    pub(super) fixed_dt: f32,
    pub(super) catch_panics: bool,
    pub(super) services: Box<dyn Services>,
    pub(super) modules: Vec<ModuleSlot<E>>,
    pub(super) module_fault_tolerance: ModuleFaultTolerance,
    pub(super) module_ids: HashSet<&'static str>,

    pub resources: Resources,
    pub(super) bus: Bus<E>,

    pub(super) events: EventHub,
    pub(super) scheduler: Scheduler,

    pub(super) plugins: PluginManager,
    pub(super) plugins_loaded: bool,
    pub(super) plugins_dir: Option<PathBuf>,

    pub(super) shutdown: ShutdownToken,
    pub(super) exit_requested: bool,

    pub(super) frame_index: u64,
    pub(super) fixed_tick: u64,
    pub(super) started: bool,
    pub(super) last: Instant,
    pub(super) acc: f32,
}

impl<E: Send + 'static> Engine<E> {
    #[inline]
    pub fn request_exit(&mut self) -> EngineResult<()> {
        self.exit_requested = true;
        self.shutdown.request();
        Ok(())
    }

    #[inline]
    pub fn shutdown_token(&self) -> ShutdownToken {
        self.shutdown.clone()
    }

    #[inline]
    pub fn events(&self) -> &EventHub {
        &self.events
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
        // Backward-compatible convenience: if the host has already resolved a `StartupConfig`
        // (via `StartupLoader`), reuse its modules directory and per-plugin overrides.
        //
        // This prevents a common integration footgun where the app loads config.json but then
        // forgets to forward plugin overrides into `EngineConfig`.
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
        // Integration footgun guard:
        // many apps build a custom `EngineConfig` and forget to forward startup-derived
        // plugin overrides (and sometimes the modules_dir) into it.
        //
        // If the app used `StartupLoader`, reuse that resolved config unless the caller
        // explicitly provided their own values.
        if let Some(startup) = crate::startup::last_startup_config() {
            if config.plugins_dir.is_none() {
                config.plugins_dir = Some(startup.modules_dir.clone());
            }

            if config.plugin_overrides.is_empty() && !startup.plugins.is_empty() {
                config.plugin_overrides = startup.plugins.clone();
            }
        }

        // Logging is provided by an optional runtime plugin (DLL).
        // No logging plugin -> no process logger installed -> all `log::*` calls become no-ops.

        let fixed_dt = (config.fixed_dt_ms as f32 / 1000.0).max(0.001);

        let mut resources = Resources::default();
        resources.insert(PluginControlQueue::default());

        init_host_context();

        // Must be available before any plugin init() runs.
        init_plugin_config_service(config.plugin_overrides.clone());

        register_engine_builtins(MathRegistry::global())
            .map_err(|e| EngineError::Other(format!("math init failed: {e}")))?;

        Ok(Self {
            fixed_dt,
            catch_panics: config.catch_panics,
            services,
            modules: Vec::new(),
            module_fault_tolerance: config.module_fault_tolerance,
            module_ids: HashSet::new(),

            resources,
            bus,
            events: EventHub::new(),
            scheduler: Scheduler::new(),

            plugins: PluginManager::new(),
            plugins_loaded: false,
            plugins_dir: config.plugins_dir,

            shutdown,
            exit_requested: false,

            frame_index: 0,
            fixed_tick: 0,
            started: false,
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
    pub(super) fn is_exit_requested(&self) -> bool {
        self.exit_requested || self.shutdown.is_requested()
    }

    #[inline]
    pub(super) fn sync_shutdown_state(&mut self) {
        if self.shutdown.is_requested() {
            self.exit_requested = true;
        }
    }

    #[inline]
    pub(super) fn propagate_shutdown_request(&mut self) {
        if self.exit_requested {
            self.shutdown.request();
        }
    }
}
