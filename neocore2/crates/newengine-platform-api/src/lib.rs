#![forbid(unsafe_op_in_unsafe_fn)]

use abi_stable::std_types::{ROption, RResult, RString, RVec};
use abi_stable::StableAbi;
use newengine_plugin_api::HostApiV1;
use serde::{Deserialize, Serialize};

/// Engine-facing platform service gateway id. Runtime/plugin consumers call this facade;
/// the host resolves it to the active platform provider or host-owned route.
pub const ENGINE_PLATFORM_SERVICE_ID: &str = "engine.platform";

/// Default/first-party provider service id for future platform backends.
pub const PLATFORM_SERVICE_ID: &str = "platform.api";
pub const PLATFORM_BACKEND_CAPABILITY_ID: &str = "platform.backend";

pub const PLATFORM_SERVICE_METHOD_INFO: &str = newengine_service_api::SERVICE_METHOD_INFO_JSON;
pub const PLATFORM_SERVICE_METHOD_INVOKE: &str = newengine_service_api::SERVICE_METHOD_INVOKE_JSON;
pub const PLATFORM_SERVICE_METHOD_SHUTDOWN_V1: &str = newengine_service_api::SERVICE_METHOD_SHUTDOWN_V1;
pub const PLATFORM_SERVICE_METHOD_WINDOW_SNAPSHOT_JSON_V1: &str = "window_snapshot_json_v1";

pub const PLATFORM_REQUIRED_METHODS_V1: &[&str] = &[
    PLATFORM_SERVICE_METHOD_INFO,
    PLATFORM_SERVICE_METHOD_INVOKE,
    PLATFORM_SERVICE_METHOD_SHUTDOWN_V1,
    PLATFORM_SERVICE_METHOD_WINDOW_SNAPSHOT_JSON_V1,
];

/// Generic backend-family declaration for platform providers.
pub const PLATFORM_BACKEND_SERVICE_SPEC: newengine_service_api::BackendServiceSpec =
    newengine_service_api::BackendServiceSpec::new(
        "platform",
        ENGINE_PLATFORM_SERVICE_ID,
        PLATFORM_SERVICE_ID,
        PLATFORM_BACKEND_CAPABILITY_ID,
    );

/// Startup validation contract for the engine-facing platform gateway.
pub const PLATFORM_RUNTIME_CONTRACT_SPEC: newengine_service_api::RuntimeServiceContractSpec =
    newengine_service_api::RuntimeServiceContractSpec::new(
        ENGINE_PLATFORM_SERVICE_ID,
        "newengine.platform-api >= 0.1.x",
        PLATFORM_REQUIRED_METHODS_V1,
    );

/// Missing platform degrades by default; strict profiles can require it.
pub const PLATFORM_RUNTIME_REQUIREMENT_SPEC: newengine_service_api::RuntimeServiceRequirementSpec =
    newengine_service_api::RuntimeServiceRequirementSpec::new(
        PLATFORM_RUNTIME_CONTRACT_SPEC,
        Some(PLATFORM_BACKEND_CAPABILITY_ID),
        Some("NEWENGINE_REQUIRE_PLATFORM_BACKEND"),
    );

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformServiceInfo {
    pub protocol: String,
    #[serde(default)]
    pub features: Vec<String>,
    #[serde(default)]
    pub methods: Vec<String>,
}

impl Default for PlatformServiceInfo {
    #[inline]
    fn default() -> Self {
        Self {
            protocol: "newengine.platform-api/v1".to_owned(),
            features: vec![
                "host-owned-window-snapshot".to_owned(),
                "native-window-handles".to_owned(),
                "surface-metrics".to_owned(),
            ],
            methods: PLATFORM_REQUIRED_METHODS_V1
                .iter()
                .map(|it| (*it).to_owned())
                .collect(),
        }
    }
}

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


#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, StableAbi, Serialize, Deserialize)]
pub enum PlatformWindowModeV1 {
    Windowed = 0,
    Borderless = 1,
    ExclusiveFullscreen = 2,
}

impl Default for PlatformWindowModeV1 {
    #[inline]
    fn default() -> Self {
        Self::Windowed
    }
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, StableAbi, Serialize, Deserialize)]
pub enum PlatformHdrModeV1 {
    Auto = 0,
    Enabled = 1,
    Disabled = 2,
}

impl Default for PlatformHdrModeV1 {
    #[inline]
    fn default() -> Self {
        Self::Auto
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, StableAbi, Serialize, Deserialize)]
pub struct PlatformDisplayConfigV1 {
    /// -1 means primary/default monitor. Non-negative values select from the
    /// platform monitor enumeration.
    pub monitor_index: i32,
    pub window_mode: PlatformWindowModeV1,
    pub vsync: bool,
    /// 0 means automatic OS/platform choice. Values are millihertz.
    pub refresh_rate_millihz: u32,
    pub render_scale: f32,
    pub hdr: PlatformHdrModeV1,
}

impl Default for PlatformDisplayConfigV1 {
    #[inline]
    fn default() -> Self {
        Self {
            monitor_index: -1,
            window_mode: PlatformWindowModeV1::Windowed,
            vsync: true,
            refresh_rate_millihz: 0,
            render_scale: 1.0,
            hdr: PlatformHdrModeV1::Auto,
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
    pub display: PlatformDisplayConfigV1,
}

impl Default for PlatformAppConfigV1 {
    #[inline]
    fn default() -> Self {
        Self {
            title: RString::from("NewEngine"),
            width: 1600,
            height: 900,
            placement: PlatformWindowPlacementV1::default(),
            icon: ROption::RNone,
            display: PlatformDisplayConfigV1::default(),
        }
    }
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
