use crate::{log::Logger, telemetry::Telemetry, time::Time};
use winit::window::Window;

#[derive(Debug, Clone)]
pub struct FrameConstitution {
    pub fixed_dt_sec: f32,
    pub max_fixed_steps_per_frame: u32,
    pub max_dt_sec: f32,

    pub log_fps: bool,
    pub fps_log_period_sec: f32,
}

impl Default for FrameConstitution {
    fn default() -> Self {
        Self {
            fixed_dt_sec: 1.0 / 60.0,
            max_fixed_steps_per_frame: 8,
            max_dt_sec: 0.25,
            log_fps: true,
            fps_log_period_sec: 1.0,
        }
    }
}

/// FrameContext — ваш “интерпретируемый” контракт.
/// Всё, что нужно модулю, должно быть тут.
/// Это и есть доказательство "движка".
pub struct FrameContext<'a> {
    pub window: &'a Window,
    pub log: &'a Logger,
    pub time: &'a mut Time,
    pub telemetry: &'a mut Telemetry,
    pub exit_requested: &'a mut bool,
}