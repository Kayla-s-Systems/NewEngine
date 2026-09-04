use serde::Serialize;
use std::path::PathBuf;

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
    pub(crate) fn new(
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
