#![forbid(unsafe_op_in_unsafe_fn)]

use serde_json::Value;
use newengine_math::collections_prelude::NeHashMap as HashMap;
use std::path::{Path, PathBuf};

use crate::log_fmt::{ellipsize, emit_boxed_kv};
use crate::path_fmt::{canonicalize_if_exists, display_clean};

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

const CACHE_FILES_CONFIG_KEYS: &[&str] = &["CACHE_FILES", "cache_files"];
const CONFIG_CONFIG_KEYS: &[&str] = &["CONFIG", "config"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StartupStorageRootKind {
    CacheFiles,
    Config,
}

impl StartupStorageRootKind {
    pub const ALL: [Self; 2] = [Self::CacheFiles, Self::Config];

    #[inline]
    pub const fn key(self) -> &'static str {
        match self {
            Self::CacheFiles => "cache_files",
            Self::Config => "config",
        }
    }

    #[inline]
    pub const fn config_keys(self) -> &'static [&'static str] {
        match self {
            Self::CacheFiles => CACHE_FILES_CONFIG_KEYS,
            Self::Config => CONFIG_CONFIG_KEYS,
        }
    }
}

#[derive(Debug, Clone)]
pub struct StartupConfig {
    pub source: StartupConfigSource,

    pub window_title: String,
    pub window_size: (u32, u32),
    pub window_placement: WindowPlacement,

    /// Logical path resolved via the AssetManager plugin VFS.
    /// Example: "ui/icons/builtin_icons.ytd@app_logo".
    pub window_icon_path: Option<String>,

    pub modules_dir: PathBuf,

    /// Engine-wide cache root. All loggers, shader bakers and cache writers
    /// must resolve their cache files through this path.
    ///
    /// CACHE_FILES is disposable generated data and may be safely cleaned.
    pub cache_files: PathBuf,

    /// Engine-wide durable user configuration root.
    ///
    /// CONFIG stores user settings such as input bindings and must not be
    /// treated as disposable cache data.
    pub config: PathBuf,


    /// Raw plugin override roots from the `plugins` object in `config.json`.
    ///
    /// Supported forms:
    /// - flat plugin ids: `plugins["engine.logging.chronicle"]`
    /// - nested domain wrappers: `plugins.engine.logging.chronicle`
    ///
    /// Resolution is performed by the plugin config service at query time.
    /// Exact flat ids take precedence, while nested domain wrappers fill missing keys.
    pub plugins: HashMap<String, Value>,

    pub extra: HashMap<String, String>,
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

            modules_dir: PathBuf::from("pluginsRuntime"),
            cache_files: PathBuf::from(crate::cache_files::DEFAULT_CACHE_FILES_DIR),
            config: PathBuf::from(crate::config_root::DEFAULT_CONFIG_DIR),


            plugins: HashMap::default(),

            extra: HashMap::default(),
        }
    }
}

impl StartupConfig {
    #[inline]
    pub fn config_base_dir(&self) -> Option<PathBuf> {
        match &self.source {
            StartupConfigSource::File { path } => path.parent().map(Path::to_path_buf),
            StartupConfigSource::Defaults => None,
        }
    }

    #[inline]
    pub fn storage_root(&self, kind: StartupStorageRootKind) -> &PathBuf {
        match kind {
            StartupStorageRootKind::CacheFiles => &self.cache_files,
            StartupStorageRootKind::Config => &self.config,
        }
    }

    #[inline]
    pub fn storage_root_mut(&mut self, kind: StartupStorageRootKind) -> &mut PathBuf {
        match kind {
            StartupStorageRootKind::CacheFiles => &mut self.cache_files,
            StartupStorageRootKind::Config => &mut self.config,
        }
    }

    #[inline]
    pub fn resolved_storage_root(&self, kind: StartupStorageRootKind) -> PathBuf {
        let base = self.config_base_dir();
        match kind {
            StartupStorageRootKind::CacheFiles => {
                crate::cache_files::normalize_cache_path(self.cache_files.clone(), base.as_deref())
            }
            StartupStorageRootKind::Config => {
                crate::config_root::normalize_config_path(self.config.clone(), base.as_deref())
            }
        }
    }

    #[inline]
    pub fn publish_storage_root_env(&self, kind: StartupStorageRootKind) -> PathBuf {
        let root = self.resolved_storage_root(kind);
        match kind {
            StartupStorageRootKind::CacheFiles => crate::cache_files::publish_cache_files_env(&root),
            StartupStorageRootKind::Config => crate::config_root::publish_config_env(&root),
        }
        root
    }

    #[inline]
    pub fn storage_child(&self, kind: StartupStorageRootKind, path: impl AsRef<Path>) -> PathBuf {
        match kind {
            StartupStorageRootKind::CacheFiles => {
                crate::cache_files::resolve_under_cache_root(&self.resolved_storage_root(kind), path.as_ref())
            }
            StartupStorageRootKind::Config => {
                crate::config_root::resolve_under_config_root(&self.resolved_storage_root(kind), path.as_ref())
            }
        }
    }

    #[inline]
    pub fn resolved_cache_files_dir(&self) -> PathBuf {
        self.resolved_storage_root(StartupStorageRootKind::CacheFiles)
    }

    #[inline]
    pub fn publish_cache_files_env(&self) -> PathBuf {
        self.publish_storage_root_env(StartupStorageRootKind::CacheFiles)
    }

    #[inline]
    pub fn resolved_config_dir(&self) -> PathBuf {
        self.resolved_storage_root(StartupStorageRootKind::Config)
    }

    #[inline]
    pub fn publish_config_env(&self) -> PathBuf {
        self.publish_storage_root_env(StartupStorageRootKind::Config)
    }

    #[inline]
    pub fn cache_child(&self, path: impl AsRef<Path>) -> PathBuf {
        self.storage_child(StartupStorageRootKind::CacheFiles, path)
    }

    #[inline]
    pub fn config_child(&self, path: impl AsRef<Path>) -> PathBuf {
        self.storage_child(StartupStorageRootKind::Config, path)
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
    /// Size of the loaded config file in bytes (if any).
    pub file_bytes: Option<usize>,
    /// Total load+parse+apply wall time in milliseconds.
    pub total_ms: Option<u32>,
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
            file_bytes: None,
            total_ms: None,
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
            StartupConfigSource::File { path } => format!("File({})", display_clean(path)),
        };

        let file = self
            .file
            .as_ref()
            .map(|p| display_clean(p))
            .unwrap_or_else(|| "<none>".to_owned());

        emit_boxed_kv(
            "StartupConfig :: Source [config applied]",
            &[
                ("source", src),
                ("file", file),
                ("resolved_from", format!("{:?}", self.resolved_from)),
                (
                    "file_bytes",
                    self.file_bytes
                        .map(|v| v.to_string())
                        .unwrap_or_else(|| "<none>".to_owned()),
                ),
                (
                    "total_ms",
                    self.total_ms
                        .map(|v| v.to_string())
                        .unwrap_or_else(|| "<none>".to_owned()),
                ),
                ("overrides", self.overrides.len().to_string()),
                ("plugin_overrides", self.plugin_overrides.len().to_string()),
            ],
        );

        let base_dir: PathBuf = self
            .file
            .as_ref()
            .and_then(|p| p.parent().map(Path::to_path_buf))
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

        let exe_dir: Option<PathBuf> = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(Path::to_path_buf));

        if !self.overrides.is_empty() {
            let rows: Vec<(&str, String)> = self
                .overrides
                .iter()
                .map(|o| {
                    let to = match o.key {
                        "modules_dir" => exe_dir
                            .as_ref()
                            .map(|d| display_clean(&canonicalize_if_exists(&d.join(&o.to))))
                            .unwrap_or_else(|| o.to.clone()),
                        "window_icon" => {
                            display_clean(&canonicalize_if_exists(&base_dir.join(&o.to)))
                        }
                        _ => o.to.clone(),
                    };
                    (o.key, format!("{} -> {}", o.from, to))
                })
                .collect();
            emit_boxed_kv("StartupConfig :: Overrides [applied]", &rows);
        }

        if !self.plugin_overrides.is_empty() {
            let rows: Vec<(&str, String)> = self
                .plugin_overrides
                .iter()
                .map(|o| {
                    (
                        o.plugin_id.as_str(),
                        format!(
                            "{} -> {}",
                            o.from,
                            summarize_plugin_override_preview(&o.to)
                        ),
                    )
                })
                .collect();
            emit_boxed_kv("StartupConfig :: Plugin Overrides [applied]", &rows);
        }
    }
}

#[derive(Debug, Clone)]
pub struct ConfigPaths {
    startup_path: String,
}

fn summarize_plugin_override_preview(value: &str) -> String {
    let compact = value.split_whitespace().collect::<Vec<_>>().join(" ");
    let preview = ellipsize(&compact, 180);
    format!("json(len={}, preview='{}')", value.len(), preview)
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
