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

/// Window icon payload (RGBA8).
#[derive(Debug, Clone)]
pub struct WinitAppIcon {
    pub rgba: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

impl WinitAppIcon {
    /// Decodes PNG bytes into RGBA8 icon.
    ///
    /// # Errors
    /// Returns error string if decoding fails.
    pub fn from_png_bytes(png: &[u8]) -> Result<Self, String> {
        let img = image::load_from_memory(png).map_err(|e| e.to_string())?;
        let rgba = img.to_rgba8();
        let (width, height) = rgba.dimensions();
        Ok(Self {
            rgba: rgba.into_raw(),
            width,
            height,
        })
    }
}

/// Winit host configuration.
#[derive(Debug, Clone)]
pub struct WinitAppConfig {
    pub title: String,
    pub size: (u32, u32),
    pub placement: WinitWindowPlacement,
    pub ui_backend: UiBackend,

    /// Optional window icon.
    pub icon: Option<WinitAppIcon>,
}

impl Default for WinitAppConfig {
    #[inline]
    fn default() -> Self {
        Self {
            title: "NewEngine".to_owned(),
            size: (1280, 720),
            placement: WinitWindowPlacement::Centered { offset: (0, 0) },
            ui_backend: UiBackend::Egui,
            icon: None,
        }
    }
}