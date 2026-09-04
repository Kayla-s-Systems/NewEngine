use serde::{Deserialize, Serialize};

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
