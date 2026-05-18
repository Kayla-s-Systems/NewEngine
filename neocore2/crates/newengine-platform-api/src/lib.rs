#![forbid(unsafe_op_in_unsafe_fn)]

use abi_stable::std_types::{ROption, RResult, RString, RVec};
use abi_stable::StableAbi;
use newengine_plugin_api::HostApiV1;
use serde::{Deserialize, Serialize};

pub const PLATFORM_WINDOW_SERVICE_ID: &str = "platform.window.v1";
pub const PLATFORM_WINDOW_BACKEND_CAPABILITY_ID: &str = "platform.window.backend";
pub const PLATFORM_WINDOW_SERVICE_METHOD_SNAPSHOT_JSON_V1: &str = "snapshot_json_v1";

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, StableAbi, Serialize, Deserialize)]
pub enum PlatformWindowPlacementKindV1 {
    OsDefault = 0,
    Centered = 1,
    Absolute = 2,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, StableAbi, Serialize, Deserialize)]
pub struct PlatformWindowPlacementV1 {
    pub kind: PlatformWindowPlacementKindV1,
    pub x: i32,
    pub y: i32,
}

impl Default for PlatformWindowPlacementV1 {
    #[inline]
    fn default() -> Self {
        Self {
            kind: PlatformWindowPlacementKindV1::OsDefault,
            x: 0,
            y: 0,
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, StableAbi)]
pub struct PlatformAppIconV1 {
    pub rgba: RVec<u8>,
    pub width: u32,
    pub height: u32,
}

#[repr(C)]
#[derive(Debug, Clone, StableAbi)]
pub struct PlatformAppConfigV1 {
    pub title: RString,
    pub width: u32,
    pub height: u32,
    pub placement: PlatformWindowPlacementV1,
    pub icon: ROption<PlatformAppIconV1>,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, StableAbi, Serialize, Deserialize)]
pub enum NativeWindowBackendV1 {
    Unknown = 0,
    Win32 = 1,
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
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, StableAbi, Serialize, Deserialize)]
pub enum PlatformCursorGrabModeV1 {
    None = 0,
    Confined = 1,
    Locked = 2,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, StableAbi)]
pub struct PlatformCursorStateV1 {
    pub visible: bool,
    pub grab: PlatformCursorGrabModeV1,
}

impl Default for PlatformCursorStateV1 {
    #[inline]
    fn default() -> Self {
        Self {
            visible: true,
            grab: PlatformCursorGrabModeV1::None,
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, StableAbi)]
pub struct PlatformCursorPollV1 {
    pub has_value: bool,
    pub state: PlatformCursorStateV1,
}

impl Default for PlatformCursorPollV1 {
    #[inline]
    fn default() -> Self {
        Self {
            has_value: false,
            state: PlatformCursorStateV1::default(),
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, PartialEq, StableAbi)]
pub struct PlatformLoadingOverlayV1 {
    pub active: bool,
    pub progress_01: f32,
    pub spinner_phase: u32,
    pub title: RString,
    pub status: RString,
    pub detail: RString,
    /// Structured system-layer overlay model serialized as JSON.
    ///
    /// The native platform loading surface uses this to render real stage cards
    /// from the runtime-host/provider model instead of inventing labels from a
    /// percentage value. Legacy shells may ignore it and keep using title/status/detail.
    pub view_json: RString,
}

impl Default for PlatformLoadingOverlayV1 {
    #[inline]
    fn default() -> Self {
        Self {
            active: false,
            progress_01: 0.0,
            spinner_phase: 0,
            title: RString::from(""),
            status: RString::from(""),
            detail: RString::from(""),
            view_json: RString::from(""),
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, PartialEq, StableAbi)]
pub struct PlatformStepResultV1 {
    pub exit_requested: bool,
    pub loading_overlay: PlatformLoadingOverlayV1,
}

impl Default for PlatformStepResultV1 {
    #[inline]
    fn default() -> Self {
        Self {
            exit_requested: false,
            loading_overlay: PlatformLoadingOverlayV1::default(),
        }
    }
}

#[repr(C)]
#[derive(Clone, StableAbi)]
pub struct PlatformHostApiV1 {
    pub user_data: usize,
    pub on_window_ready_v1:
        extern "C" fn(usize, PlatformWindowReadyV1) -> RResult<(), RString>,
    pub on_window_resized_v1:
        extern "C" fn(usize, PlatformSurfaceMetricsV1) -> RResult<(), RString>,
    pub on_window_focused_v1: extern "C" fn(usize, bool) -> RResult<(), RString>,
    pub on_close_requested_v1: extern "C" fn(usize) -> RResult<(), RString>,
    pub step_v1: extern "C" fn(usize, f32) -> RResult<PlatformStepResultV1, RString>,
    pub poll_cursor_state_v1: extern "C" fn(usize) -> PlatformCursorPollV1,
}

pub type PlatformRunResultV1 = RResult<(), RString>;
pub type PlatformRuntimeRunFnV1 =
unsafe extern "C" fn(HostApiV1, PlatformHostApiV1, PlatformAppConfigV1) -> PlatformRunResultV1;
