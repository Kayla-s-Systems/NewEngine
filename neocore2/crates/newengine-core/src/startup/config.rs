#![forbid(unsafe_op_in_unsafe_fn)]

use serde_json::Value;
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

    pub window_title: String,
    pub window_size: (u32, u32),
    pub window_placement: WindowPlacement,

    /// Path inside assets root, resolved via AssetManager + existing importers.
    /// Example: "ui/icon.png".
    pub window_icon_path: Option<String>,

    pub modules_dir: PathBuf,

    pub render_backend: String,
    pub render_clear_color: [f32; 4],
    pub render_debug_text: String,

    pub ui_backend: UiBackend,

    /// Per-plugin override objects (the `plugins` object from config.json).
    ///
    /// Key: plugin id (e.g. "newengine.logging")
    /// Value: JSON object with overrides.
    pub plugins: HashMap<String, Value>,

    pub extra: HashMap<String, String>,

    /// Legacy (kept for backward compat). Prefer `window_icon_path`.
    pub window_icon_png: Option<Vec<u8>>,
}

impl Default for StartupConfig {
    #[inline]
    fn default() -> Self {
        Self {
            source: StartupConfigSource::Defaults,

            window_title: "NewEngine".to_owned(),
            window_size: (1600, 900),
            window_placement: WindowPlacement::Default,

            window_icon_path: None,

            modules_dir: PathBuf::from("./"),

            render_backend: "vulkan".to_owned(),
            render_clear_color: [0.02, 0.02, 0.03, 1.0],
            render_debug_text: "NewEngine".to_owned(),

            ui_backend: UiBackend::default(),

            plugins: HashMap::new(),

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
pub struct StartupPluginOverride {
    pub plugin_id: String,
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
    pub plugin_overrides: Vec<StartupPluginOverride>,
}

impl StartupLoadReport {
    #[inline]
    pub fn new() -> Self {
        Self {
            source: StartupConfigSource::Defaults,
            file: None,
            resolved_from: StartupResolvedFrom::NotProvided,
            overrides: Vec::new(),
            plugin_overrides: Vec::new(),
        }
    }

    /// Emits a deterministic override summary via the global `log` facade.
    ///
    /// Intended usage: call this AFTER plugins are loaded so the logging plugin
    /// (if present) can capture the output. If no logging plugin exists, the
    /// host is expected to install a no-op logger and nothing will be printed.
    pub fn emit_logs(&self) {
        let src = match &self.source {
            StartupConfigSource::Defaults => "Defaults".to_owned(),
            StartupConfigSource::File { path } => format!("File({})", path.display()),
        };

        let file = self
            .file
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "<none>".to_owned());

        log::info!(
            "startup: loaded source={} file={} resolved_from={:?} overrides={} plugin_overrides={}",
            src,
            file,
            self.resolved_from,
            self.overrides.len(),
            self.plugin_overrides.len()
        );

        for o in &self.overrides {
            log::info!("startup: override {}: '{}' -> '{}'", o.key, o.from, o.to);
        }

        for o in &self.plugin_overrides {
            log::info!(
                "startup: plugin override {}: '{}' -> '{}'",
                o.plugin_id,
                o.from,
                o.to
            );
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
