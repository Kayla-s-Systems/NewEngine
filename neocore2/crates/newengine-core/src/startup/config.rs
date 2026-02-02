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

/// Where a single override came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StartupOverrideSource {
    Defaults,
    File,
    Env,
    Programmatic,
}

impl Default for StartupOverrideSource {
    #[inline]
    fn default() -> Self {
        Self::Defaults
    }
}

#[derive(Debug, Clone)]
pub struct StartupConfig {
    pub source: StartupConfigSource,

    // Logging
    pub log_level: String,

    // Window
    pub window_title: String,
    pub window_size: (u32, u32),
    pub window_placement: WindowPlacement,

    // Engine
    pub fixed_dt_ms: u32,
    pub assets_root: PathBuf,
    pub asset_budget: u32,
    pub init_host_context: bool,

    // Modules / plugins
    pub modules_dir: PathBuf,

    // Extra
    pub extra: HashMap<String, String>,
}


impl Default for StartupConfig {
    fn default() -> Self {
        Self {
            source: StartupConfigSource::Defaults,

            log_level: "info".to_owned(),

            window_title: "NewEngine".to_owned(),
            window_size: (1600, 900),
            window_placement: WindowPlacement::Default,

            fixed_dt_ms: 16,
            assets_root: PathBuf::from("assets"),
            asset_budget: 8,
            init_host_context: true,

            modules_dir: PathBuf::from("modules"),

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

    pub fixed_dt_ms: Option<u32>,
    pub assets_root: Option<PathBuf>,
    pub asset_budget: Option<u32>,
    pub init_host_context: Option<bool>,

    pub modules_dir: Option<PathBuf>,
}

/// Programmatic overrides and env overrides share the same shape.
/// The loader applies layers in this order:
/// defaults -> file -> env -> programmatic.
#[derive(Debug, Clone, Default)]
pub struct StartupOverrides {
    pub log_level: Option<String>,
    pub window_title: Option<String>,
    pub window_size: Option<(u32, u32)>,
    pub window_placement: Option<WindowPlacement>,

    pub fixed_dt_ms: Option<u32>,
    pub assets_root: Option<PathBuf>,
    pub asset_budget: Option<u32>,
    pub init_host_context: Option<bool>,

    pub modules_dir: Option<PathBuf>,
    pub extra: HashMap<String, String>,
}

impl StartupOverrides {
    #[inline]
    pub fn empty() -> Self {
        Self::default()
    }

    /// Loads overrides from environment variables.
    ///
    /// Supported variables:
    /// - NEWENGINE_LOG_LEVEL
    /// - NEWENGINE_WINDOW_TITLE
    /// - NEWENGINE_WINDOW_SIZE (e.g. "1600x900")
    /// - NEWENGINE_WINDOW_WIDTH + NEWENGINE_WINDOW_HEIGHT
    /// - NEWENGINE_WINDOW_PLACEMENT ("default" | "centered")
    /// - NEWENGINE_WINDOW_OFFSET (e.g. "0,-24") used only for "centered"
    /// - NEWENGINE_FIXED_DT_MS
    /// - NEWENGINE_ASSETS_ROOT
    /// - NEWENGINE_ASSET_BUDGET
    /// - NEWENGINE_INIT_HOST_CONTEXT ("true" | "false" | "1" | "0")
    /// - NEWENGINE_MODULES_DIR
    /// - NEWENGINE_EXTRA_* (arbitrary; key becomes lowercase with '.' separators)
    pub fn from_env() -> Self {
        fn get(k: &str) -> Option<String> {
            std::env::var(k)
                .ok()
                .map(|v| v.trim().to_owned())
                .filter(|v| !v.is_empty())
        }

        let mut out = Self::default();

        out.log_level = get("NEWENGINE_LOG_LEVEL");
        out.window_title = get("NEWENGINE_WINDOW_TITLE");

        let w_size = get("NEWENGINE_WINDOW_SIZE");
        let w_w = get("NEWENGINE_WINDOW_WIDTH");
        let w_h = get("NEWENGINE_WINDOW_HEIGHT");

        if let Some(s) = w_size {
            if let Some((w, h)) = parse_u32_pair(&s, 'x') {
                out.window_size = Some((w, h));
            }
        } else if let (Some(sw), Some(sh)) = (w_w, w_h) {
            if let (Ok(w), Ok(h)) = (sw.parse::<u32>(), sh.parse::<u32>()) {
                out.window_size = Some((w, h));
            }
        }

        let placement = get("NEWENGINE_WINDOW_PLACEMENT")
            .unwrap_or_else(|| "".to_owned())
            .to_ascii_lowercase();

        if !placement.is_empty() {
            match placement.as_str() {
                "default" => out.window_placement = Some(WindowPlacement::Default),
                "centered" => {
                    let off = get("NEWENGINE_WINDOW_OFFSET")
                        .and_then(|s| parse_i32_pair(&s, ','))
                        .unwrap_or((0, 0));

                    out.window_placement = Some(WindowPlacement::Centered { offset: off });
                }
                _ => {}
            }
        }

        out.fixed_dt_ms = get("NEWENGINE_FIXED_DT_MS").and_then(|v| v.parse::<u32>().ok());
        out.assets_root = get("NEWENGINE_ASSETS_ROOT").map(PathBuf::from);
        out.asset_budget = get("NEWENGINE_ASSET_BUDGET").and_then(|v| v.parse::<u32>().ok());
        out.init_host_context = get("NEWENGINE_INIT_HOST_CONTEXT").and_then(parse_bool);
        out.modules_dir = get("NEWENGINE_MODULES_DIR").map(PathBuf::from);

        for (k, v) in std::env::vars() {
            if let Some(rest) = k.strip_prefix("NEWENGINE_EXTRA_") {
                if v.trim().is_empty() {
                    continue;
                }
                let key = rest
                    .to_ascii_lowercase()
                    .replace("__", ".")
                    .replace('_', ".");
                out.extra.insert(key, v);
            }
        }

        out
    }
}

fn parse_bool(v: String) -> Option<bool> {
    let s = v.trim().to_ascii_lowercase();
    match s.as_str() {
        "1" | "true" | "yes" | "y" | "on" => Some(true),
        "0" | "false" | "no" | "n" | "off" => Some(false),
        _ => None,
    }
}

fn parse_u32_pair(s: &str, sep: char) -> Option<(u32, u32)> {
    let mut it = s.split(sep);
    let a = it.next()?.trim().parse::<u32>().ok()?;
    let b = it.next()?.trim().parse::<u32>().ok()?;
    if it.next().is_some() {
        return None;
    }
    Some((a, b))
}

fn parse_i32_pair(s: &str, sep: char) -> Option<(i32, i32)> {
    let mut it = s.split(sep);
    let a = it.next()?.trim().parse::<i32>().ok()?;
    let b = it.next()?.trim().parse::<i32>().ok()?;
    if it.next().is_some() {
        return None;
    }
    Some((a, b))
}

#[derive(Debug, Clone)]
pub struct StartupOverride {
    pub key: &'static str,
    pub source: StartupOverrideSource,
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