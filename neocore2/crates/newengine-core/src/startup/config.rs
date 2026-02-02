use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct ConfigPaths {
    pub startup: PathBuf,
    pub root_dir: Option<PathBuf>,
}

impl Default for ConfigPaths {
    #[inline]
    fn default() -> Self {
        Self {
            startup: PathBuf::from("config.json"),
            root_dir: None,
        }
    }
}

impl ConfigPaths {
    /// Universal constructor. App may pass PathBuf, String, &str, etc.
    #[inline]
    pub fn new<P>(startup: P, root_dir: Option<PathBuf>) -> Self
    where
        P: Into<PathBuf>,
    {
        Self {
            startup: startup.into(),
            root_dir,
        }
    }

    /// Uses default startup file name ("config.json") and optional root directory.
    #[inline]
    pub fn with_root(root_dir: Option<PathBuf>) -> Self {
        Self {
            startup: PathBuf::from("config.json"),
            root_dir,
        }
    }

    /// "One-liner" for main.rs.
    #[inline]
    pub fn from_startup_str(startup: &str) -> Self {
        Self::new(startup, None)
    }

    #[inline]
    pub fn with_root_dir(mut self, root_dir: impl Into<PathBuf>) -> Self {
        self.root_dir = Some(root_dir.into());
        self
    }

    #[inline]
    pub fn startup_path(&self) -> &Path {
        &self.startup
    }
}

#[derive(Debug, Clone)]
pub enum StartupConfigSource {
    Defaults,
    File { path: PathBuf },
    Mixed,
}

impl Default for StartupConfigSource {
    #[inline]
    fn default() -> Self {
        Self::Defaults
    }
}

/// Window placement policy (boot-level).
#[derive(Debug, Clone)]
pub enum WindowPlacement {
    /// Center window on the current monitor, applying offset in physical pixels.
    Centered { offset: (i32, i32) },
    /// Platform/default placement.
    Default,
}

impl Default for WindowPlacement {
    #[inline]
    fn default() -> Self {
        Self::Default
    }
}

/// Normalized startup configuration.
/// All fields have concrete defaults (no Option).
#[derive(Debug, Clone)]
pub struct StartupConfig {
    pub source: StartupConfigSource,

    pub log_level: String,
    pub window_title: String,
    pub window_size: (u32, u32),
    pub window_placement: WindowPlacement,

    pub modules_dir: PathBuf,

    pub assets_root: PathBuf,
    pub asset_pump_steps: u32,
    pub asset_filesystem_source: bool,

    pub render_backend: String,
    pub render_clear_color: [f32; 4],
    pub render_debug_text: String,

    pub extra: HashMap<String, String>,
}

impl Default for StartupConfig {
    #[inline]
    fn default() -> Self {
        Self {
            source: StartupConfigSource::Defaults,

            log_level: "info".to_owned(),
            window_title: "NewEngine".to_owned(),
            window_size: (1600, 900),
            window_placement: WindowPlacement::Default,

            modules_dir: PathBuf::from("./"),

            assets_root: PathBuf::from("assets"),
            asset_pump_steps: 8,
            asset_filesystem_source: true,

            render_backend: "vulkan".to_owned(),
            render_clear_color: [0.02, 0.02, 0.03, 1.0],
            render_debug_text: "NewEngine".to_owned(),

            extra: HashMap::new(),
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
    /// Path was absolute and existed.
    Absolute,
    /// Found as `cwd/<file>`.
    Cwd,
    /// Found as `exe_dir/<file>`.
    ExeDir,
    /// Found as `root_dir/<file>`.
    RootDir,
    /// Found as-is (relative) in OS resolution.
    AsIs,
    /// No file path was provided.
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
    /// The actual file used (absolute when found).
    pub file: Option<PathBuf>,
    /// Where the file was resolved from.
    pub resolved_from: StartupResolvedFrom,
    pub overrides: Vec<StartupOverride>,
}

impl StartupLoadReport {
    #[inline]
    pub fn has_overrides(&self) -> bool {
        !self.overrides.is_empty()
    }

    #[inline]
    pub fn is_defaults(&self) -> bool {
        matches!(self.source, StartupConfigSource::Defaults)
    }

    #[inline]
    pub fn used_file(&self) -> Option<&Path> {
        self.file.as_deref()
    }
}