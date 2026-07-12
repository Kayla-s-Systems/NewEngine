use abi_stable::StableAbi;
use serde::{Deserialize, Serialize};

use crate::PlatformDisplayConfigV1;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, StableAbi, Serialize, Deserialize)]
pub enum NativeWindowBackendV1 {
    Unknown = 0,
    Win32 = 1,
    /// Wayland: display = wl_display*, window = wl_surface*.
    Wayland = 2,
    /// Xlib: display = Display* or 0, window = X11 Window, reserved0 = screen, reserved1 = visual_id.
    Xlib = 3,
    /// Xcb: display = xcb_connection_t* or 0, window = xcb_window_t, reserved0 = screen, reserved1 = visual_id or 0.
    Xcb = 4,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, StableAbi, Serialize, Deserialize)]
pub struct NativeWindowHandlesV1 {
    pub backend: NativeWindowBackendV1,
    pub window: u64,
    pub display: u64,
    pub reserved0: u64,
    pub reserved1: u64,
}

impl Default for NativeWindowHandlesV1 {
    #[inline]
    fn default() -> Self {
        Self {
            backend: NativeWindowBackendV1::Unknown,
            window: 0,
            display: 0,
            reserved0: 0,
            reserved1: 0,
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, StableAbi, Serialize, Deserialize)]
pub struct PlatformSurfaceMetricsV1 {
    pub width: u32,
    pub height: u32,
    pub pixels_per_point: f32,
}

impl Default for PlatformSurfaceMetricsV1 {
    #[inline]
    fn default() -> Self {
        Self {
            width: 0,
            height: 0,
            pixels_per_point: 1.0,
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, StableAbi, Serialize, Deserialize)]
pub struct PlatformWindowReadyV1 {
    pub handles: NativeWindowHandlesV1,
    pub surface: PlatformSurfaceMetricsV1,
    /// Display/presentation policy selected by the platform provider.
    #[serde(default)]
    pub display: PlatformDisplayConfigV1,
}
