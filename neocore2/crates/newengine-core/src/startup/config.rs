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
pub struct StartupConfig {
    pub source: StartupConfigSource,

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