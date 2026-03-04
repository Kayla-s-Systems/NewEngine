use serde_json::Value;

use std::collections::HashMap;
use std::path::PathBuf;

use crate::startup::StartupConfig;


#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModuleFaultTolerance {
    /// Fail-fast: any module init/start/update error aborts the engine.
    Strict,
    /// Host-first: module failures disable the module (warn/error) and the engine keeps running.
    Resilient,
}

impl Default for ModuleFaultTolerance {
    #[inline]
    fn default() -> Self {
        Self::Resilient
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginFaultTolerance {
    /// Fail-fast: any plugin load error aborts the engine start.
    Strict,
    /// Best-effort: plugin load errors are logged and ignored; the engine keeps running.
    Resilient,
}

impl Default for PluginFaultTolerance {
    #[inline]
    fn default() -> Self {
        Self::Resilient
    }
}

#[derive(Debug, Clone)]
pub struct EngineConfig {
    pub fixed_dt_ms: u32,
    pub plugins_dir: Option<PathBuf>,

    /// Per-plugin override objects, taken from `config.json.plugins`.
    ///
    /// The host exposes these overrides to plugins via the `newengine.config.v1` service.
    /// Plugins are expected to merge overrides into their own base configuration.
    pub plugin_overrides: HashMap<String, Value>,

    /// Controls how the engine handles module failures (missing deps / errors).
    pub module_fault_tolerance: ModuleFaultTolerance,

    /// Controls how the engine handles plugin load failures (invalid DLL, missing symbols, init errors).
    ///
    /// Note: "plugin absent" (not present on disk) is not an error; it's just a degraded capability set.
    pub plugin_fault_tolerance: PluginFaultTolerance,

    /// Controls how the engine reacts to panics inside module callbacks.
    ///
    /// - When `true` (default), the engine converts panics to `EngineError` and requests shutdown.
    /// - When `false`, panics unwind normally (useful for debugging).
    pub catch_panics: bool,
}

impl Default for EngineConfig {
    #[inline]
    fn default() -> Self {
        Self {
            fixed_dt_ms: 16,
            plugins_dir: None,
            plugin_overrides: HashMap::new(),
            module_fault_tolerance: ModuleFaultTolerance::Resilient,
            plugin_fault_tolerance: PluginFaultTolerance::Resilient,
            catch_panics: true,
        }
    }
}

impl EngineConfig {
    #[inline]
    pub fn new(fixed_dt_ms: u32) -> Self {
        Self {
            fixed_dt_ms,
            plugins_dir: None,
            plugin_overrides: HashMap::new(),
            module_fault_tolerance: ModuleFaultTolerance::Resilient,
            plugin_fault_tolerance: PluginFaultTolerance::Resilient,
            catch_panics: true,
        }
    }

    /// Builds an engine config from a resolved startup config.
    ///
    /// The startup config is the single source of truth for:
    /// - module scan directory (`modules_dir`)
    /// - per-plugin override objects (`plugins`)
    #[inline]
    pub fn from_startup(startup: &StartupConfig, fixed_dt_ms: u32) -> Self {
        Self::new(fixed_dt_ms)
            .with_plugins_dir(Some(startup.modules_dir.clone()))
            .with_plugin_overrides(startup.plugins.clone())
    }

    #[inline]
    pub fn with_plugins_dir(mut self, dir: Option<PathBuf>) -> Self {
        self.plugins_dir = dir;
        self
    }

    #[inline]
    pub fn with_plugin_overrides(mut self, overrides: HashMap<String, Value>) -> Self {
        self.plugin_overrides = overrides;
        self
    }

    #[inline]
    pub fn with_module_fault_tolerance(mut self, mode: ModuleFaultTolerance) -> Self {
        self.module_fault_tolerance = mode;
        self
    }

    #[inline]
    pub fn with_plugin_fault_tolerance(mut self, mode: PluginFaultTolerance) -> Self {
        self.plugin_fault_tolerance = mode;
        self
    }

    #[inline]
    pub fn with_catch_panics(mut self, enabled: bool) -> Self {
        self.catch_panics = enabled;
        self
    }
}
