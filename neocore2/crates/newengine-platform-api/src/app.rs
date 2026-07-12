use abi_stable::std_types::{ROption, RString, RVec};
use abi_stable::StableAbi;
use serde::{Deserialize, Serialize};

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
