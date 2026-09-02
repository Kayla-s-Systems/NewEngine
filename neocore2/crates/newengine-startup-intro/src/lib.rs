#![forbid(unsafe_op_in_unsafe_fn)]

use serde::{Deserialize, Serialize};
use std::sync::OnceLock;
use std::{
    env, fs,
    path::{Path, PathBuf},
};

pub const STARTUP_INTRO_SCHEMA: &str = "newengine.startup_intro.v1";
pub const STARTUP_INTRO_SKIP_ENV: &str = "NEWENGINE_STARTUP_INTRO_SKIP";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct StartupIntroManifest {
    pub format_version: u32,
    pub schema: String,
    pub enabled: bool,
    pub window: StartupIntroWindow,
    pub sequence: Vec<StartupIntroEntry>,
}

impl Default for StartupIntroManifest {
    fn default() -> Self {
        Self {
            format_version: 1,
            schema: STARTUP_INTRO_SCHEMA.to_owned(),
            enabled: true,
            window: StartupIntroWindow::default(),
            sequence: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct StartupIntroWindow {
    pub mode: String,
    pub width: u32,
    pub height: u32,
    pub background: String,
    pub topmost: bool,
    pub failure_timeout_ms: u64,
}

impl Default for StartupIntroWindow {
    fn default() -> Self {
        Self {
            mode: "fullscreen".to_owned(),
            width: 1280,
            height: 720,
            background: "#000000".to_owned(),
            topmost: false,
            failure_timeout_ms: 30_000,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct StartupIntroEntry {
    pub id: String,
    pub source: String,
    pub enabled: bool,
    pub skippable: bool,
    pub volume: f32,
    pub max_duration_ms: Option<u64>,
}

impl Default for StartupIntroEntry {
    fn default() -> Self {
        Self {
            id: String::new(),
            source: String::new(),
            enabled: true,
            skippable: true,
            volume: 1.0,
            max_duration_ms: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StartupIntroStatus {
    Played,
    Disabled,
    Skipped,
    Empty,
    Unavailable,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StartupIntroReport {
    pub status: StartupIntroStatus,
    pub descriptor_path: PathBuf,
    pub entries: usize,
    pub detail: String,
}

impl StartupIntroReport {
    fn new(
        status: StartupIntroStatus,
        descriptor_path: PathBuf,
        entries: usize,
        detail: impl Into<String>,
    ) -> Self {
        Self {
            status,
            descriptor_path,
            entries,
            detail: detail.into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ResolvedStartupIntro {
    pub window: ResolvedStartupIntroWindow,
    pub sequence: Vec<ResolvedStartupIntroEntry>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ResolvedStartupIntroWindow {
    pub mode: String,
    pub width: u32,
    pub height: u32,
    pub background: String,
    pub topmost: bool,
    pub failure_timeout_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ResolvedStartupIntroEntry {
    pub id: String,
    pub source: String,
    pub skippable: bool,
    pub volume: f32,
    pub max_duration_ms: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum StartupIntroNativeBackend {
    Unknown,
    Win32,
    Wayland,
    Xlib,
    Xcb,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct StartupIntroNativeWindow {
    pub backend: StartupIntroNativeBackend,
    pub window: u64,
    pub display: u64,
}

impl StartupIntroNativeWindow {
    #[inline]
    pub const fn new(backend: StartupIntroNativeBackend, window: u64, display: u64) -> Self {
        Self {
            backend,
            window,
            display,
        }
    }
}

/// Host/provider presentation port. The contract crate owns sequencing and validation.
/// Presentation always targets an already-created game window; providers must never
/// create a second splash window.
pub type StartupIntroPresenterFn =
    fn(&ResolvedStartupIntro, StartupIntroNativeWindow) -> Result<(), String>;

static STARTUP_INTRO_PRESENTER: OnceLock<StartupIntroPresenterFn> = OnceLock::new();

/// Installs the process startup-intro presenter. Registration is deterministic and first-wins.
pub fn install_startup_intro_presenter(presenter: StartupIntroPresenterFn) -> bool {
    STARTUP_INTRO_PRESENTER.set(presenter).is_ok()
}

#[inline]
pub fn startup_intro_presenter_registered() -> bool {
    STARTUP_INTRO_PRESENTER.get().is_some()
}

pub fn play_from_descriptor_in_window(
    descriptor_path: impl AsRef<Path>,
    root_dir: impl AsRef<Path>,
    target: StartupIntroNativeWindow,
) -> StartupIntroReport {
    let descriptor_path = descriptor_path.as_ref().to_path_buf();

    if env_bool(STARTUP_INTRO_SKIP_ENV, false) {
        return StartupIntroReport::new(
            StartupIntroStatus::Skipped,
            descriptor_path,
            0,
            format!("startup intro suppressed by {STARTUP_INTRO_SKIP_ENV}"),
        );
    }

    let manifest = match load_manifest(&descriptor_path) {
        Ok(manifest) => manifest,
        Err(error) => {
            return StartupIntroReport::new(
                StartupIntroStatus::Unavailable,
                descriptor_path,
                0,
                error,
            )
        }
    };
    if !manifest.enabled {
        return StartupIntroReport::new(
            StartupIntroStatus::Disabled,
            descriptor_path,
            0,
            "startup intro descriptor is disabled",
        );
    }

    let payload = match resolve_payload(&manifest, &descriptor_path, root_dir.as_ref()) {
        Ok(payload) => payload,
        Err(error) => {
            return StartupIntroReport::new(
                StartupIntroStatus::Unavailable,
                descriptor_path,
                0,
                error,
            )
        }
    };
    if payload.sequence.is_empty() {
        return StartupIntroReport::new(
            StartupIntroStatus::Empty,
            descriptor_path,
            0,
            "startup intro sequence has no enabled entries",
        );
    }

    let entry_count = payload.sequence.len();
    let Some(presenter) = STARTUP_INTRO_PRESENTER.get().copied() else {
        return StartupIntroReport::new(
            StartupIntroStatus::Unavailable,
            descriptor_path,
            entry_count,
            "startup intro was requested, but no presenter provider is registered",
        );
    };
    match presenter(&payload, target) {
        Ok(()) => StartupIntroReport::new(
            StartupIntroStatus::Played,
            descriptor_path,
            entry_count,
            format!(
                "played {entry_count} startup intro entr{}",
                if entry_count == 1 { "y" } else { "ies" }
            ),
        ),
        Err(error) => StartupIntroReport::new(
            StartupIntroStatus::Unavailable,
            descriptor_path,
            entry_count,
            error,
        ),
    }
}

pub fn resolve_descriptor_path(raw: &str, runtime_config_path: &Path, root_dir: &Path) -> PathBuf {
    let raw = raw.trim();
    if let Some(relative) = raw
        .strip_prefix("ROOT-DIR/")
        .or_else(|| raw.strip_prefix("ROOT-DIR\\"))
    {
        return root_dir.join(relative.replace('/', std::path::MAIN_SEPARATOR_STR));
    }
    let path = PathBuf::from(raw);
    if path.is_absolute() {
        path
    } else {
        runtime_config_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(path)
    }
}

fn load_manifest(path: &Path) -> Result<StartupIntroManifest, String> {
    let source = fs::read_to_string(path).map_err(|error| {
        format!(
            "read startup intro descriptor '{}': {error}",
            path.display()
        )
    })?;
    let manifest: StartupIntroManifest = toml::from_str(&source).map_err(|error| {
        format!(
            "parse startup intro descriptor '{}': {error}",
            path.display()
        )
    })?;
    validate_manifest(&manifest)?;
    Ok(manifest)
}

fn validate_manifest(manifest: &StartupIntroManifest) -> Result<(), String> {
    if manifest.format_version != 1 {
        return Err(format!(
            "startup intro format_version must be 1; actual={}",
            manifest.format_version
        ));
    }
    if manifest.schema.trim() != STARTUP_INTRO_SCHEMA {
        return Err(format!(
            "startup intro schema must be '{}'; actual='{}'",
            STARTUP_INTRO_SCHEMA, manifest.schema
        ));
    }
    let mode = manifest.window.mode.trim();
    if !matches!(mode, "fullscreen" | "windowed") {
        return Err(format!(
            "startup intro window.mode must be 'fullscreen' or 'windowed'; actual='{mode}'"
        ));
    }
    if manifest.window.failure_timeout_ms == 0 {
        return Err("startup intro window.failure_timeout_ms must be greater than zero".to_owned());
    }
    for (index, entry) in manifest.sequence.iter().enumerate() {
        if entry.enabled && entry.source.trim().is_empty() {
            return Err(format!(
                "startup intro sequence[{index}].source must not be empty"
            ));
        }
        if !(0.0..=1.0).contains(&entry.volume) {
            return Err(format!(
                "startup intro sequence[{index}].volume must be within 0.0..=1.0"
            ));
        }
    }
    Ok(())
}

fn resolve_payload(
    manifest: &StartupIntroManifest,
    descriptor_path: &Path,
    root_dir: &Path,
) -> Result<ResolvedStartupIntro, String> {
    let descriptor_dir = descriptor_path.parent().unwrap_or_else(|| Path::new("."));
    let sequence = manifest
        .sequence
        .iter()
        .filter(|entry| entry.enabled)
        .map(|entry| {
            let source = resolve_media_path(entry.source.trim(), descriptor_dir, root_dir);
            if !source.is_file() {
                return Err(format!(
                    "startup intro media '{}' for entry '{}' does not exist",
                    source.display(),
                    entry.id
                ));
            }
            Ok(ResolvedStartupIntroEntry {
                id: entry.id.trim().to_owned(),
                source: source.to_string_lossy().into_owned(),
                skippable: entry.skippable,
                volume: entry.volume.clamp(0.0, 1.0),
                max_duration_ms: entry
                    .max_duration_ms
                    .unwrap_or(manifest.window.failure_timeout_ms)
                    .max(1),
            })
        })
        .collect::<Result<Vec<_>, String>>()?;

    Ok(ResolvedStartupIntro {
        window: ResolvedStartupIntroWindow {
            mode: manifest.window.mode.trim().to_owned(),
            width: manifest.window.width.max(1),
            height: manifest.window.height.max(1),
            background: manifest.window.background.trim().to_owned(),
            topmost: manifest.window.topmost,
            failure_timeout_ms: manifest.window.failure_timeout_ms.max(1),
        },
        sequence,
    })
}

fn resolve_media_path(raw: &str, descriptor_dir: &Path, root_dir: &Path) -> PathBuf {
    if let Some(relative) = raw
        .strip_prefix("ROOT-DIR/")
        .or_else(|| raw.strip_prefix("ROOT-DIR\\"))
    {
        return root_dir.join(relative.replace('/', std::path::MAIN_SEPARATOR_STR));
    }
    let path = PathBuf::from(raw);
    if path.is_absolute() {
        path
    } else {
        descriptor_dir.join(path)
    }
}

fn env_bool(name: &str, default: bool) -> bool {
    env::var(name)
        .ok()
        .map(|value| match value.trim().to_ascii_lowercase().as_str() {
            "1" | "true" | "yes" | "on" => true,
            "0" | "false" | "no" | "off" => false,
            _ => default,
        })
        .unwrap_or(default)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_supports_an_ordered_logo_sequence() {
        let source = r#"
format_version = 1
schema = "newengine.startup_intro.v1"
enabled = true

[window]
mode = "fullscreen"

[[sequence]]
id = "northstar"
source = "logo.mp4"

[[sequence]]
id = "middleware"
source = "middleware.mp4"
skippable = false
volume = 0.5
"#;
        let manifest: StartupIntroManifest = toml::from_str(source).unwrap();
        validate_manifest(&manifest).unwrap();
        assert_eq!(manifest.sequence.len(), 2);
        assert_eq!(manifest.sequence[0].id, "northstar");
        assert_eq!(manifest.sequence[1].volume, 0.5);
        assert!(!manifest.sequence[1].skippable);
    }

    #[test]
    fn root_dir_token_is_data_driven() {
        let root = Path::new("C:/NorthStar");
        let runtime = Path::new("C:/NorthStar/NewEngine/neocore2/runtime.toml");
        let resolved = resolve_descriptor_path(
            "ROOT-DIR/Shared/Source/authoring/northstar/intro/intro.toml",
            runtime,
            root,
        );
        assert_eq!(
            resolved,
            root.join("Shared/Source/authoring/northstar/intro/intro.toml")
        );
    }
}
