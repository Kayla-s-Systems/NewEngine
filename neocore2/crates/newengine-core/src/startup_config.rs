use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default)]
pub struct ConfigPaths {
    pub startup: Option<PathBuf>,
    pub root_dir: Option<PathBuf>,
}

impl ConfigPaths {
    /// Universal constructor. App may pass PathBuf, String, &str, etc.
    #[inline]
    pub fn new<P>(startup: P, root_dir: Option<PathBuf>) -> Self
    where
        P: Into<PathBuf>,
    {
        Self {
            startup: Some(startup.into()),
            root_dir,
        }
    }

    #[inline]
    pub fn none(root_dir: Option<PathBuf>) -> Self {
        Self {
            startup: None,
            root_dir,
        }
    }

    /// "One-liner" for main.rs.
    #[inline]
    pub fn from_startup_str(startup: &str) -> Self {
        Self::new(startup, None)
    }

    /// Optional startup path helper.
    #[inline]
    pub fn startup_optional(startup: Option<&str>, root_dir: Option<PathBuf>) -> Self {
        Self {
            startup: startup.map(PathBuf::from),
            root_dir,
        }
    }

    #[inline]
    pub fn with_root_dir(mut self, root_dir: impl Into<PathBuf>) -> Self {
        self.root_dir = Some(root_dir.into());
        self
    }

    #[inline]
    pub fn startup_path(&self) -> Option<&Path> {
        self.startup.as_deref()
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

#[derive(Debug, Clone)]
pub struct StartupConfig {
    pub source: StartupConfigSource,

    pub log_level: Option<String>,
    pub window_title: Option<String>,
    pub window_size: Option<(u32, u32)>,
    pub window_placement: Option<WindowPlacement>,
    pub modules_dir: Option<PathBuf>,

    pub extra: HashMap<String, String>,
}

impl Default for StartupConfig {
    #[inline]
    fn default() -> Self {
        Self {
            source: StartupConfigSource::Defaults,
            log_level: None,
            window_title: None,
            window_size: None,
            window_placement: None,
            modules_dir: None,
            extra: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct StartupDefaults {
    pub log_level: Option<String>,
    pub window_title: Option<String>,
    pub window_size: Option<(u32, u32)>,
    pub window_placement: Option<WindowPlacement>,
    pub modules_dir: Option<PathBuf>,
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