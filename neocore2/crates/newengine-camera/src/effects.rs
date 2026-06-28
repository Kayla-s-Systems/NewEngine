#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_math::Vec2;

use crate::CameraFrame;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// Depth-of-field planes carried with the camera frame.
///
/// The reference camera keeps a regular frame and a post-effect frame. NewEngine keeps the same
/// separation by treating DOF, blur and post-process flags as frame sidecar data instead of letting
/// render code infer them from gameplay state.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct CameraDepthOfFieldSettings {
    pub near_start: f32,
    pub near_end: f32,
    pub far_start: f32,
    pub far_end: f32,
    pub blend_level: f32,
}

impl Default for CameraDepthOfFieldSettings {
    #[inline]
    fn default() -> Self {
        Self {
            near_start: 0.0,
            near_end: 0.0,
            far_start: 10_000.0,
            far_end: 10_000.0,
            blend_level: 0.0,
        }
    }
}

impl CameraDepthOfFieldSettings {
    #[inline]
    pub fn sanitized(self) -> Self {
        let near_start = finite_or(self.near_start, 0.0).max(0.0);
        let near_end = finite_or(self.near_end, near_start).max(near_start);
        let far_start = finite_or(self.far_start, near_end).max(near_end);
        let far_end = finite_or(self.far_end, far_start).max(far_start);
        Self {
            near_start,
            near_end,
            far_start,
            far_end,
            blend_level: finite_or(self.blend_level, 0.0).clamp(0.0, 1.0),
        }
    }

    #[inline]
    pub const fn force_high_quality(mut self) -> Self {
        self.blend_level = 1.0;
        self
    }
}

/// Motion blur parameters associated with a camera output frame.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct CameraMotionBlurSettings {
    pub strength: f32,
    pub decay_rate: f32,
}

impl Default for CameraMotionBlurSettings {
    #[inline]
    fn default() -> Self {
        Self {
            strength: 0.0,
            decay_rate: 0.5,
        }
    }
}

impl CameraMotionBlurSettings {
    #[inline]
    pub fn sanitized(self) -> Self {
        Self {
            strength: finite_or(self.strength, 0.0).clamp(0.0, 1.0),
            decay_rate: finite_or(self.decay_rate, 0.5).clamp(0.0, 1.0),
        }
    }

    #[inline]
    pub fn decay_from(self, previous_strength: f32) -> Self {
        let this = self.sanitized();
        let prev = finite_or(previous_strength, this.strength).clamp(0.0, 1.0);
        if this.strength >= prev {
            return this;
        }
        let strength = prev - ((prev - this.strength) * this.decay_rate);
        Self { strength, ..this }
    }
}

/// Post-effect state resolved after a camera/director update.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct CameraPostEffects {
    pub dof: CameraDepthOfFieldSettings,
    pub motion_blur: CameraMotionBlurSettings,
    /// Extra screen-space shake amplitude after the spatial frame was resolved.
    pub shake_amplitude: f32,
    /// Optional renderer-facing exposure bias in stops.
    pub exposure_bias: f32,
    /// Jitter override/addition in pixels for downstream temporal effects.
    pub jitter_px: Vec2,
    pub high_quality_dof: bool,
}

impl Default for CameraPostEffects {
    #[inline]
    fn default() -> Self {
        Self {
            dof: CameraDepthOfFieldSettings::default(),
            motion_blur: CameraMotionBlurSettings::default(),
            shake_amplitude: 0.0,
            exposure_bias: 0.0,
            jitter_px: Vec2::ZERO,
            high_quality_dof: false,
        }
    }
}

impl CameraPostEffects {
    #[inline]
    pub fn sanitized(self) -> Self {
        Self {
            dof: if self.high_quality_dof {
                self.dof.force_high_quality().sanitized()
            } else {
                self.dof.sanitized()
            },
            motion_blur: self.motion_blur.sanitized(),
            shake_amplitude: finite_or(self.shake_amplitude, 0.0).max(0.0),
            exposure_bias: finite_or(self.exposure_bias, 0.0).clamp(-16.0, 16.0),
            jitter_px: Vec2::new(
                finite_or(self.jitter_px.x, 0.0),
                finite_or(self.jitter_px.y, 0.0),
            ),
            high_quality_dof: self.high_quality_dof,
        }
    }

    #[inline]
    pub fn with_motion_blur(mut self, strength: f32) -> Self {
        self.motion_blur.strength = strength;
        self
    }

    #[inline]
    pub fn force_high_quality_dof(mut self) -> Self {
        self.high_quality_dof = true;
        self.dof.blend_level = 1.0;
        self
    }
}

/// Full camera frame plus the post-effect sidecar.
#[derive(Clone, Copy, Debug)]
pub struct CameraResolvedFrame {
    pub frame: CameraFrame,
    pub effects: CameraPostEffects,
}

impl CameraResolvedFrame {
    #[inline]
    pub fn new(frame: CameraFrame) -> Self {
        Self {
            frame,
            effects: CameraPostEffects::default(),
        }
    }

    #[inline]
    pub fn with_effects(frame: CameraFrame, effects: CameraPostEffects) -> Self {
        Self {
            frame,
            effects: effects.sanitized(),
        }
    }
}

#[inline]
fn finite_or(value: f32, fallback: f32) -> f32 {
    if value.is_finite() {
        value
    } else {
        fallback
    }
}
