#![forbid(unsafe_op_in_unsafe_fn)]

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// Stable camera ownership lane.
///
/// This mirrors the reference architecture idea of explicit camera channels/directors:
/// gameplay, editor, cinematic, debug, scripted and replay cameras must not compete through
/// ad-hoc globals. The runtime chooses one rendered channel per viewport and may blend between
/// channel frames later.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum CameraChannel {
    Gameplay,
    Editor,
    Debug,
    Cinematic,
    Cutscene,
    Scripted,
    Replay,
    UiPreview,
}

impl Default for CameraChannel {
    #[inline]
    fn default() -> Self {
        Self::Gameplay
    }
}

/// Deterministic channel blend metadata.
///
/// A frame with weight `1.0` is fully dominant. Values below one are reserved for future
/// switch/cutscene/replay blending without changing the renderer-facing frame contract.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct CameraChannelState {
    pub channel: CameraChannel,
    pub weight: f32,
    pub priority: i32,
}

impl CameraChannelState {
    #[inline]
    pub const fn new(channel: CameraChannel, weight: f32, priority: i32) -> Self {
        Self { channel, weight, priority }
    }

    #[inline]
    pub const fn dominant(channel: CameraChannel) -> Self {
        Self { channel, weight: 1.0, priority: 0 }
    }
}

impl Default for CameraChannelState {
    #[inline]
    fn default() -> Self {
        Self::dominant(CameraChannel::Gameplay)
    }
}
