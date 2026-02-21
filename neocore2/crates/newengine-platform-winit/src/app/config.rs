#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_core::startup::UiBackend;
use std::time::Duration;

use newengine_assets::{wait_ready, AssetAccess, AssetServiceClient};
use newengine_core::plugins::default_host_api;

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
    /// Decodes common image formats (PNG/ICO) into RGBA8 icon.
    ///
    /// # Errors
    /// Returns error string if decoding fails.
    pub fn from_image_bytes(bytes: &[u8]) -> Result<Self, String> {
        let img = image::load_from_memory(bytes).map_err(|e| e.to_string())?;
        let rgba = img.to_rgba8();
        let (width, height) = rgba.dimensions();
        Ok(Self {
            rgba: rgba.into_raw(),
            width,
            height,
        })
    }

    /// Decodes PNG bytes into RGBA8 icon.
    ///
    /// Prefer [`Self::from_image_bytes`] for ICO support.
    #[inline]
    pub fn from_png_bytes(png: &[u8]) -> Result<Self, String> {
        Self::from_image_bytes(png)
    }

    /// Builds an icon from an AssetManager `blob_wire_v1` payload.
    ///
    /// The AssetManager may return either:
    /// - raw `RGBA8` payload (common for imported textures), in which case `meta_json`
    ///   should contain `width`/`height` (or similarly named fields), or
    /// - encoded image bytes (PNG/ICO), in which case we fall back to
    ///   [`Self::from_image_bytes`].
    pub fn from_asset_blob(meta_json: &str, payload: &[u8]) -> Result<Self, String> {
        let (w, h) = parse_width_height(meta_json);

        if let (Some(width), Some(height)) = (w, h) {
            let expected = (width as usize)
                .checked_mul(height as usize)
                .and_then(|v| v.checked_mul(4))
                .ok_or_else(|| "icon: width/height overflow".to_string())?;

            if payload.len() == expected {
                return Ok(Self {
                    rgba: payload.to_vec(),
                    width,
                    height,
                });
            }
        }

        Self::from_image_bytes(payload)
    }
}

#[inline]
fn parse_width_height(meta_json: &str) -> (Option<u32>, Option<u32>) {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(meta_json) else {
        return (None, None);
    };

    // Be permissive: different importer versions may use different field names.
    let w = v
        .get("width")
        .or_else(|| v.get("w"))
        .or_else(|| v.get("image_width"))
        .and_then(|x| x.as_u64())
        .and_then(|x| u32::try_from(x).ok());

    let h = v
        .get("height")
        .or_else(|| v.get("h"))
        .or_else(|| v.get("image_height"))
        .and_then(|x| x.as_u64())
        .and_then(|x| u32::try_from(x).ok());

    (w, h)
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

impl WinitAppConfig {
    /// Loads the window icon through the AssetManager service and installs it into this config.
    ///
    /// This is the intended path for packed builds where the icon lives inside VFS layers (.pak).
    ///
    /// # Errors
    /// Returns an error string if the asset cannot be loaded, imported, or decoded.
    pub fn with_icon_from_assets(mut self, logical_path: &str, timeout: Duration) -> Result<Self, String> {
        let assets = AssetServiceClient::new(default_host_api());
        let id = assets.load(logical_path)?;

        wait_ready(&assets, &id, timeout)
            .map_err(|e| format!("window icon: wait_ready failed path='{logical_path}' err='{e:?}'"))?;

        let (meta_json, payload) = assets.blob_wire_v1(&id)?;
        let icon = WinitAppIcon::from_asset_blob(&meta_json, &payload)
            .map_err(|e| format!("window icon: decode failed path='{logical_path}' err='{e}'"))?;

        self.icon = Some(icon);
        Ok(self)
    }
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