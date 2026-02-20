use crate::startup::StartupLoggingConfig;

use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct EngineConfig {
    pub fixed_dt_ms: u32,
    pub plugins_dir: Option<PathBuf>,

    /// Optional startup logging configuration.
    ///
    /// The engine applies this configuration to NEWENGINE_LOG_* environment variables
    /// deterministically during `Engine::new_with_config`.
    /// Actual logging is expected to be initialized by a runtime logging plugin (DLL).
    pub startup_logging: Option<StartupLoggingConfig>,

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
            startup_logging: Some(StartupLoggingConfig::auto()),
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
            startup_logging: None,
            catch_panics: true,
        }
    }

    #[inline]
    pub fn with_startup_logging(mut self, cfg: StartupLoggingConfig) -> Self {
        self.startup_logging = Some(cfg);
        self
    }

    #[inline]
    pub fn with_plugins_dir(mut self, dir: Option<PathBuf>) -> Self {
        self.plugins_dir = dir;
        self
    }

    #[inline]
    pub fn with_catch_panics(mut self, enabled: bool) -> Self {
        self.catch_panics = enabled;
        self
    }
}
