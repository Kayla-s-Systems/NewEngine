use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineConfig {
    #[serde(default)]
    pub window: WindowConfig,
    #[serde(default)]
    pub frame: FrameConfig,
    #[serde(default)]
    pub runtime: RuntimeConfig,
    #[serde(default)]
    pub modules: Vec<ModuleConfig>,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            window: WindowConfig::default(),
            frame: FrameConfig::default(),
            runtime: RuntimeConfig::default(),
            modules: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowConfig {
    #[serde(default = "default_title")]
    pub title: String,
    #[serde(default = "default_width")]
    pub width: u32,
    #[serde(default = "default_height")]
    pub height: u32,
}

fn default_title() -> String { "NEOCORE2".to_string() }
fn default_width() -> u32 { 1280 }
fn default_height() -> u32 { 720 }

impl Default for WindowConfig {
    fn default() -> Self {
        Self { title: default_title(), width: default_width(), height: default_height() }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrameConfig {
    #[serde(default = "default_fixed_hz")]
    pub fixed_hz: u32,
    #[serde(default = "default_max_fixed_steps")]
    pub max_fixed_steps_per_frame: u32,
    #[serde(default = "default_max_dt_ms")]
    pub max_dt_ms: u32,
    #[serde(default)]
    pub log_fps: bool,
    #[serde(default = "default_fps_period_ms")]
    pub fps_log_period_ms: u32,
}

fn default_fixed_hz() -> u32 { 60 }
fn default_max_fixed_steps() -> u32 { 8 }
fn default_max_dt_ms() -> u32 { 250 }
fn default_fps_period_ms() -> u32 { 1000 }

impl Default for FrameConfig {
    fn default() -> Self {
        Self {
            fixed_hz: default_fixed_hz(),
            max_fixed_steps_per_frame: default_max_fixed_steps(),
            max_dt_ms: default_max_dt_ms(),
            log_fps: true,
            fps_log_period_ms: default_fps_period_ms(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeConfig {
    /// "poll" or "wait"
    #[serde(default = "default_control_flow")]
    pub control_flow: String,
}

fn default_control_flow() -> String { "poll".to_string() }

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self { control_flow: default_control_flow() }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleConfig {
    pub id: String,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(default = "default_settings")]
    pub settings: toml::Value,
}

fn default_enabled() -> bool { true }

fn default_settings() -> toml::Value {
    // Пустая таблица: безопасно для try_into() и удобно для конфигов.
    toml::Value::Table(toml::map::Map::new())
}