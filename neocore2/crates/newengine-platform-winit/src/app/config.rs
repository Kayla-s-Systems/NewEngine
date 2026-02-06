#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_core::startup::UiBackend;

/// Window placement policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WinitWindowPlacement {
    /// Let the OS decide.
    OsDefault,
    /// Place the window in the center of the primary monitor.
    Centered { offset: (i32, i32) },
    /// Absolute position in desktop coordinates.
    Absolute { x: i32, y: i32 },
}

/// Winit host configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WinitAppConfig {
    pub title: String,
    pub size: (u32, u32),
    pub placement: WinitWindowPlacement,
    pub ui_backend: UiBackend,
}

impl Default for WinitAppConfig {
    #[inline]
    fn default() -> Self {
        Self {
            title: "NewEngine".to_owned(),
            size: (1280, 720),
            placement: WinitWindowPlacement::Centered { offset: (0, 0) },
            ui_backend: UiBackend::Egui,
        }
    }
}