#![forbid(unsafe_op_in_unsafe_fn)]

use crate::{CameraChannel, CameraChannelState, Projection};

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// Stable, data-driven object identity for camera definitions.
///
/// This is deliberately small: content/profile systems can map their own asset ids into this
/// contract without pulling runtime renderer or gameplay code into the camera domain crate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct CameraObjectId(pub u64);

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct CameraObjectMetadata {
    pub id: CameraObjectId,
    pub name: String,
    pub can_pause: bool,
}

impl CameraObjectMetadata {
    #[inline]
    pub fn new(id: u64, name: impl Into<String>) -> Self {
        Self {
            id: CameraObjectId(id),
            name: name.into(),
            can_pause: true,
        }
    }

    #[inline]
    pub fn with_pause_policy(mut self, can_pause: bool) -> Self {
        self.can_pause = can_pause;
        self
    }
}

impl Default for CameraObjectMetadata {
    #[inline]
    fn default() -> Self {
        Self::new(0, "camera")
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum CameraModeKind {
    RuntimeOrbit,
    RuntimeFly,
    GameplayFirstPerson,
    GameplayThirdPersonFollow,
    GameplayThirdPersonAim,
    Cinematic,
    Scripted,
    Replay,
    DebugFree,
}

impl Default for CameraModeKind {
    #[inline]
    fn default() -> Self {
        Self::RuntimeOrbit
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum CameraContextKind {
    Runtime,
    OnFoot,
    Aiming,
    Vehicle,
    Spectator,
    Cutscene,
    Scripted,
    Replay,
    Debug,
}

impl Default for CameraContextKind {
    #[inline]
    fn default() -> Self {
        Self::Runtime
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct CameraBlendSpec {
    pub blend_in_sec: f32,
    pub blend_out_sec: f32,
    pub lock_input: bool,
    pub preserve_source_frame: bool,
}

impl Default for CameraBlendSpec {
    #[inline]
    fn default() -> Self {
        Self {
            blend_in_sec: 0.18,
            blend_out_sec: 0.14,
            lock_input: false,
            preserve_source_frame: true,
        }
    }
}

impl CameraBlendSpec {
    #[inline]
    pub const fn cut() -> Self {
        Self {
            blend_in_sec: 0.0,
            blend_out_sec: 0.0,
            lock_input: false,
            preserve_source_frame: false,
        }
    }

    #[inline]
    pub fn sanitized(self) -> Self {
        Self {
            blend_in_sec: finite_non_negative(self.blend_in_sec),
            blend_out_sec: finite_non_negative(self.blend_out_sec),
            lock_input: self.lock_input,
            preserve_source_frame: self.preserve_source_frame,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct CameraLensMetadata {
    pub projection: Projection,
    pub motion_blur_decay: f32,
}

impl CameraLensMetadata {
    #[inline]
    pub fn new(projection: Projection) -> Self {
        Self {
            projection,
            motion_blur_decay: 0.5,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct CameraDirectorMetadata {
    pub object: CameraObjectMetadata,
    pub mode: CameraModeKind,
    pub context: CameraContextKind,
    pub channel: CameraChannelState,
    pub priority: i32,
    pub blend: CameraBlendSpec,
}

impl CameraDirectorMetadata {
    #[inline]
    pub fn new(id: u64, name: impl Into<String>, mode: CameraModeKind) -> Self {
        let channel = match mode {
            CameraModeKind::Cinematic => CameraChannel::Cinematic,
            CameraModeKind::Scripted => CameraChannel::Scripted,
            CameraModeKind::Replay => CameraChannel::Replay,
            CameraModeKind::DebugFree => CameraChannel::Debug,
            CameraModeKind::RuntimeOrbit | CameraModeKind::RuntimeFly => CameraChannel::Runtime,
            _ => CameraChannel::Gameplay,
        };
        Self {
            object: CameraObjectMetadata::new(id, name),
            mode,
            context: CameraContextKind::Runtime,
            channel: CameraChannelState::dominant(channel),
            priority: default_priority_for_channel(channel),
            blend: CameraBlendSpec::default(),
        }
    }

    #[inline]
    pub fn with_context(mut self, context: CameraContextKind) -> Self {
        self.context = context;
        self
    }

    #[inline]
    pub fn with_priority(mut self, priority: i32) -> Self {
        self.priority = priority;
        self.channel.priority = priority;
        self
    }

    #[inline]
    pub fn with_blend(mut self, blend: CameraBlendSpec) -> Self {
        self.blend = blend.sanitized();
        self
    }
}

#[inline]
fn default_priority_for_channel(channel: CameraChannel) -> i32 {
    match channel {
        CameraChannel::Cutscene => 900,
        CameraChannel::Cinematic => 800,
        CameraChannel::Scripted => 700,
        CameraChannel::Replay => 650,
        CameraChannel::Debug => 600,
        CameraChannel::Gameplay => 400,
        CameraChannel::Runtime => 300,
        CameraChannel::UiPreview => 100,
    }
}

#[inline]
fn finite_non_negative(v: f32) -> f32 {
    if v.is_finite() && v > 0.0 {
        v
    } else {
        0.0
    }
}
