#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_math::{Vec2, Vec3};

use crate::{
    blend_camera_projection, CameraPostEffects, CameraResolvedFrame, CameraRig,
};

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum CameraInterpolationCurve {
    Linear,
    Acceleration,
    Deceleration,
    SmoothStep,
    EaseInOut,
}

impl Default for CameraInterpolationCurve {
    #[inline]
    fn default() -> Self {
        Self::SmoothStep
    }
}

impl CameraInterpolationCurve {
    #[inline]
    pub fn sample(self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        match self {
            Self::Linear => t,
            Self::Acceleration => t * t,
            Self::Deceleration => 1.0 - (1.0 - t) * (1.0 - t),
            Self::SmoothStep => t * t * (3.0 - 2.0 * t),
            Self::EaseInOut => {
                if t < 0.5 {
                    2.0 * t * t
                } else {
                    1.0 - (-2.0 * t + 2.0).powi(2) * 0.5
                }
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct CameraInterpolationSpec {
    pub duration_sec: f32,
    pub curve: CameraInterpolationCurve,
    pub lock_source: bool,
}

impl Default for CameraInterpolationSpec {
    #[inline]
    fn default() -> Self {
        Self {
            duration_sec: 0.18,
            curve: CameraInterpolationCurve::SmoothStep,
            lock_source: false,
        }
    }
}

impl CameraInterpolationSpec {
    #[inline]
    pub const fn cut() -> Self {
        Self {
            duration_sec: 0.0,
            curve: CameraInterpolationCurve::Linear,
            lock_source: false,
        }
    }

    #[inline]
    pub fn sanitized(self) -> Self {
        Self {
            duration_sec: if self.duration_sec.is_finite() && self.duration_sec > 0.0 {
                self.duration_sec
            } else {
                0.0
            },
            curve: self.curve,
            lock_source: self.lock_source,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct CameraFrameInterpolator {
    pub spec: CameraInterpolationSpec,
    pub elapsed_sec: f32,
    pub source: Option<CameraResolvedFrame>,
    pub target: Option<CameraResolvedFrame>,
    pub active: bool,
}

impl Default for CameraFrameInterpolator {
    #[inline]
    fn default() -> Self {
        Self {
            spec: CameraInterpolationSpec::cut(),
            elapsed_sec: 0.0,
            source: None,
            target: None,
            active: false,
        }
    }
}

impl CameraFrameInterpolator {
    #[inline]
    pub fn begin(
        &mut self,
        source: CameraResolvedFrame,
        target: CameraResolvedFrame,
        spec: CameraInterpolationSpec,
    ) {
        let spec = spec.sanitized();
        self.spec = spec;
        self.elapsed_sec = 0.0;
        self.source = Some(source);
        self.target = Some(target);
        self.active = spec.duration_sec > 0.0;
    }

    #[inline]
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    #[inline]
    pub fn is_active(&self) -> bool {
        self.active
    }

    #[inline]
    pub fn alpha(&self) -> f32 {
        if !self.active || self.spec.duration_sec <= 0.0 {
            1.0
        } else {
            self.spec.curve.sample(self.elapsed_sec / self.spec.duration_sec)
        }
    }

    #[inline]
    pub fn update(&mut self, next_target: CameraResolvedFrame, dt: f32) -> CameraResolvedFrame {
        if !self.active {
            self.target = Some(next_target);
            return next_target;
        }

        if !self.spec.lock_source {
            self.target = Some(next_target);
        }

        if dt.is_finite() && dt > 0.0 {
            self.elapsed_sec += dt;
        }

        let alpha = self.alpha();
        let source = self.source.unwrap_or(next_target);
        let target = self.target.unwrap_or(next_target);
        let output = blend_resolved_camera_frames(source, target, alpha);

        if alpha >= 1.0 {
            self.active = false;
            self.source = None;
            self.elapsed_sec = 0.0;
        }

        output
    }
}

#[inline]
pub fn blend_resolved_camera_frames(
    from: CameraResolvedFrame,
    to: CameraResolvedFrame,
    t: f32,
) -> CameraResolvedFrame {
    let t = t.clamp(0.0, 1.0);
    let rig = blend_camera_rig(from.frame.rig, to.frame.rig, t);
    let projection = blend_camera_projection(from.frame.projection, to.frame.projection, t);
    let jitter_px = lerp_vec2(from.frame.jitter_px, to.frame.jitter_px, t);
    let frame = crate::CameraFrame::build(to.frame.channel, rig, projection, to.frame.viewport, jitter_px);
    let effects = blend_camera_post_effects(from.effects, to.effects, t);
    CameraResolvedFrame::with_effects(frame, effects)
}

#[inline]
pub fn blend_camera_rig(from: CameraRig, to: CameraRig, t: f32) -> CameraRig {
    let t = t.clamp(0.0, 1.0);
    CameraRig {
        position: lerp_vec3(from.position, to.position, t),
        rotation: from.rotation.slerp(to.rotation, t).normalize_or_identity(),
    }
}

#[inline]
pub fn blend_camera_post_effects(
    from: CameraPostEffects,
    to: CameraPostEffects,
    t: f32,
) -> CameraPostEffects {
    let t = t.clamp(0.0, 1.0);
    CameraPostEffects {
        dof: crate::CameraDepthOfFieldSettings {
            near_start: lerp_f32(from.dof.near_start, to.dof.near_start, t),
            near_end: lerp_f32(from.dof.near_end, to.dof.near_end, t),
            far_start: lerp_f32(from.dof.far_start, to.dof.far_start, t),
            far_end: lerp_f32(from.dof.far_end, to.dof.far_end, t),
            blend_level: lerp_f32(from.dof.blend_level, to.dof.blend_level, t),
        },
        motion_blur: crate::CameraMotionBlurSettings {
            strength: lerp_f32(from.motion_blur.strength, to.motion_blur.strength, t),
            decay_rate: lerp_f32(from.motion_blur.decay_rate, to.motion_blur.decay_rate, t),
        },
        shake_amplitude: lerp_f32(from.shake_amplitude, to.shake_amplitude, t),
        exposure_bias: lerp_f32(from.exposure_bias, to.exposure_bias, t),
        jitter_px: lerp_vec2(from.jitter_px, to.jitter_px, t),
        high_quality_dof: if t < 0.5 { from.high_quality_dof } else { to.high_quality_dof },
    }
    .sanitized()
}

#[inline]
fn lerp_f32(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

#[inline]
fn lerp_vec2(a: Vec2, b: Vec2, t: f32) -> Vec2 {
    Vec2::new(lerp_f32(a.x, b.x, t), lerp_f32(a.y, b.y, t))
}

#[inline]
fn lerp_vec3(a: Vec3, b: Vec3, t: f32) -> Vec3 {
    Vec3::new(lerp_f32(a.x, b.x, t), lerp_f32(a.y, b.y, t), lerp_f32(a.z, b.z, t))
}
