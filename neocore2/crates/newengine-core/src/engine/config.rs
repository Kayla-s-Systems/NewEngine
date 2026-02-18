use crate::startup::StartupLoggingConfig;

use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct EngineConfig {
    pub fixed_dt_ms: u32,
    pub plugins_dir: Option<PathBuf>,

    /// Optional startup logging configuration (process-wide).
    ///
    /// If provided, the engine initializes logging during `Engine::new_with_config`.
    /// The returned handle is kept alive by the engine instance.
    pub startup_logging: Option<StartupLoggingConfig>,

    /// Legacy log level fallback for older startup configs.
    pub legacy_log_level: Option<String>,

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
            legacy_log_level: None,
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
            legacy_log_level: None,
            catch_panics: true,
        }
    }

    #[inline]
    pub fn with_startup_logging(
        mut self,
        cfg: StartupLoggingConfig,
        legacy_level: Option<String>,
    ) -> Self {
        self.startup_logging = Some(cfg);
        self.legacy_log_level = legacy_level;
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
