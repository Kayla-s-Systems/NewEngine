#![forbid(unsafe_op_in_unsafe_fn)]

use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub enum StartupConfigSource {
    Defaults,
    File { path: PathBuf },
}

impl Default for StartupConfigSource {
    #[inline]
    fn default() -> Self {
        Self::Defaults
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UiBackend {
    Disabled,
    Egui,
    Custom(String),
}

impl Default for UiBackend {
    #[inline]
    fn default() -> Self {
        Self::Egui
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowPlacement {
    Default,
    Centered { offset: (i32, i32) },
}

impl Default for WindowPlacement {
    #[inline]
    fn default() -> Self {
        Self::Default
    }
}

#[derive(Debug, Clone)]
pub struct StartupLoggingConfig {
    /// Equivalent to env_logger filter string.
    /// Example: "info,newengine_render=debug,wgpu=warn".
    pub filter: Option<String>,

    /// Default level when `filter` is not provided.
    pub level: String,

    /// "auto" | "always" | "never"
    pub style: Option<String>,

    /// Enable ANSI colors in console output.
    pub colors: bool,

    pub include_module_path: bool,
    pub include_target: bool,
    pub include_file: bool,
    pub include_line_number: bool,

    /// "seconds" | "millis" | "micros" | "nanos" | "none"
    pub timestamp: Option<String>,

    pub indent: Option<usize>,

    /// "stdout" | "stderr"
    pub console_target: Option<String>,

    /// Optional file path for log output.
    pub file_path: Option<String>,

    /// If true and file_path is Some -> console + file.
    /// If false and file_path is Some -> file only.
    pub tee: bool,

    /// Rotate when file grows beyond this size (bytes).
    pub roll_max_bytes: Option<u64>,
    /// Max number of size-rolled backups (path.1..path.N).
    pub roll_max_files: usize,
    /// Keep last N day-files (UTC epoch day) if daily rolling is enabled.
    pub roll_keep_days: Option<usize>,
}

impl Default for StartupLoggingConfig {
    #[inline]
    fn default() -> Self {
        Self {
            filter: None,
            level: "info".to_owned(),
            style: None,
            colors: true,
            include_module_path: true,
            include_target: true,
            include_file: false,
            include_line_number: false,
            timestamp: Some("millis".to_owned()),
            indent: None,
            console_target: Some("stderr".to_owned()),
            file_path: None,
            tee: false,
            roll_max_bytes: None,
            roll_max_files: 5,
            roll_keep_days: None,
        }
    }
}


impl StartupLoggingConfig {
    /// Builds a sane default logging config for the whole process.
    ///
    /// Environment variables (if present) override defaults:
    /// - NEWENGINE_LOG: full filter spec (e.g. "info,newengine_render=debug")
    /// - NEWENGINE_LOG_LEVEL: default level if NEWENGINE_LOG is not set
    /// - NEWENGINE_LOG_FILE: log file path
    /// - NEWENGINE_LOG_TEE: "true|false" (when file is enabled)
    /// - NEWENGINE_LOG_TARGET: "stdout|stderr"
    /// - NEWENGINE_LOG_STYLE: "auto|always|never"
    /// - NEWENGINE_LOG_COLORS: "true|false"
    pub fn auto() -> Self {
        use std::env;

        fn parse_bool(v: Option<String>, default: bool) -> bool {
            match v.as_deref().map(str::trim).map(str::to_ascii_lowercase) {
                Some(s) if s == "1" || s == "true" || s == "yes" || s == "on" => true,
                Some(s) if s == "0" || s == "false" || s == "no" || s == "off" => false,
                _ => default,
            }
        }

        let mut cfg = Self::default();

        cfg.filter = env::var("NEWENGINE_LOG").ok().and_then(|s| {
            let t = s.trim().to_owned();
            (!t.is_empty()).then_some(t)
        });

        if cfg.filter.is_none() {
            cfg.level = env::var("NEWENGINE_LOG_LEVEL")
                .ok()
                .and_then(|s| {
                    let t = s.trim().to_owned();
                    (!t.is_empty()).then_some(t)
                })
                .unwrap_or_else(|| cfg.level.clone());
        }

        cfg.style = env::var("NEWENGINE_LOG_STYLE").ok().and_then(|s| {
            let t = s.trim().to_owned();
            (!t.is_empty()).then_some(t)
        });

        cfg.colors = parse_bool(env::var("NEWENGINE_LOG_COLORS").ok(), cfg.colors);

        cfg.console_target = env::var("NEWENGINE_LOG_TARGET").ok().and_then(|s| {
            let t = s.trim().to_owned();
            (!t.is_empty()).then_some(t)
        });

        cfg.file_path = env::var("NEWENGINE_LOG_FILE").ok().and_then(|s| {
            let t = s.trim().to_owned();
            (!t.is_empty()).then_some(t)
        });

        // Engine default: always have a file unless user explicitly disables it.
        // You can disable file logging by setting NEWENGINE_LOG_FILE="" (empty) or "none".
        if cfg.file_path.as_deref().map(|s| s.eq_ignore_ascii_case("none")).unwrap_or(false) {
            cfg.file_path = None;
        }
        if cfg.file_path.is_none() {
            cfg.file_path = Some("logs/newengine.log".to_owned());
        }

        cfg.tee = parse_bool(env::var("NEWENGINE_LOG_TEE").ok(), true);

        cfg
    }
}


#[derive(Debug, Clone)]
pub struct StartupConfig {
    pub source: StartupConfigSource,

    /// Extended logging configuration.
    pub logging: StartupLoggingConfig,

    /// Legacy (kept for backward compat). Prefer `logging.level` and `logging.filter`.
    pub log_level: String,
    pub window_title: String,
    pub window_size: (u32, u32),
    pub window_placement: WindowPlacement,

    /// Path inside assets root, resolved via AssetManager + existing importers.
    /// Example: "ui/icon.png".
    pub window_icon_path: Option<String>,

    pub modules_dir: PathBuf,

    pub assets_root: PathBuf,
    pub asset_pump_steps: u32,
    pub asset_filesystem_source: bool,

    pub render_backend: String,
    pub render_clear_color: [f32; 4],
    pub render_debug_text: String,

    pub ui_backend: UiBackend,

    pub extra: HashMap<String, String>,

    /// Legacy (kept for backward compat). Prefer `window_icon_path`.
    pub window_icon_png: Option<Vec<u8>>,
}

impl Default for StartupConfig {
    #[inline]
    fn default() -> Self {
        Self {
            source: StartupConfigSource::Defaults,

            logging: StartupLoggingConfig::default(),

            log_level: "info".to_owned(),
            window_title: "NewEngine".to_owned(),
            window_size: (1600, 900),
            window_placement: WindowPlacement::Default,

            window_icon_path: None,

            modules_dir: PathBuf::from("./"),

            assets_root: PathBuf::from("assets"),
            asset_pump_steps: 8,
            asset_filesystem_source: true,

            render_backend: "vulkan".to_owned(),
            render_clear_color: [0.02, 0.02, 0.03, 1.0],
            render_debug_text: "NewEngine".to_owned(),

            ui_backend: UiBackend::default(),

            extra: HashMap::new(),

            window_icon_png: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct StartupOverride {
    pub key: &'static str,
    pub from: String,
    pub to: String,
}

#[derive(Debug, Clone)]
pub enum StartupResolvedFrom {
    Absolute,
    Cwd,
    ExeDir,
    RootDir,
    AsIs,
    NotProvided,
}

impl Default for StartupResolvedFrom {
    #[inline]
    fn default() -> Self {
        Self::NotProvided
    }
}

#[derive(Debug, Clone)]
pub struct StartupLoadReport {
    pub source: StartupConfigSource,
    pub file: Option<PathBuf>,
    pub resolved_from: StartupResolvedFrom,
    pub overrides: Vec<StartupOverride>,
}

impl StartupLoadReport {
    #[inline]
    pub fn new() -> Self {
        Self {
            source: StartupConfigSource::Defaults,
            file: None,
            resolved_from: StartupResolvedFrom::NotProvided,
            overrides: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ConfigPaths {
    startup_path: String,
}

impl ConfigPaths {
    #[inline]
    pub fn from_startup_str(path: &str) -> Self {
        Self {
            startup_path: path.to_owned(),
        }
    }

    #[inline]
    pub fn startup_path(&self) -> &str {
        &self.startup_path
    }
}