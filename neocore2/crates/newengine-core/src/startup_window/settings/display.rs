#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct StartupDisplaySettings {
    pub monitor_index: i32,
    pub window_mode: StartupWindowMode,
    pub vsync: bool,
    pub refresh_rate_millihz: u32,
    pub render_scale: f32,
    pub hdr: StartupHdrMode,
    /// 0 means uncapped. The active platform/runtime may clamp further.
    pub frame_limit: u32,
    pub center_window: bool,
}

impl Default for StartupDisplaySettings {
    fn default() -> Self {
        Self {
            monitor_index: -1,
            window_mode: StartupWindowMode::Windowed,
            vsync: true,
            refresh_rate_millihz: 0,
            render_scale: 1.0,
            hdr: StartupHdrMode::Auto,
            frame_limit: 0,
            center_window: true,
        }
    }
}
