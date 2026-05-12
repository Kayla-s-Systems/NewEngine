#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_camera::{
    CameraFrame, CameraRig, Orthographic, Perspective, Projection,
};
use newengine_math::{Vec2, Vec3};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CameraFrameBlendCurve {
    Linear,
    SmoothStep,
    EaseInOut,
}

impl Default for CameraFrameBlendCurve {
    #[inline]
    fn default() -> Self {
        Self::SmoothStep
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CameraFrameBlendPolicy {
    Cut,
    Blend,
}

impl Default for CameraFrameBlendPolicy {
    #[inline]
    fn default() -> Self {
        Self::Blend
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CameraFrameBlendPlan {
    pub policy: CameraFrameBlendPolicy,
    pub curve: CameraFrameBlendCurve,
    pub duration_sec: f32,
    pub lock_input: bool,
    pub preserve_previous_frame: bool,
}

impl CameraFrameBlendPlan {
    #[inline]
    pub const fn cut() -> Self {
        Self {
            policy: CameraFrameBlendPolicy::Cut,
            curve: CameraFrameBlendCurve::Linear,
            duration_sec: 0.0,
            lock_input: false,
            preserve_previous_frame: false,
        }
    }

    #[inline]
    pub const fn timed(duration_sec: f32, curve: CameraFrameBlendCurve) -> Self {
        Self {
            policy: CameraFrameBlendPolicy::Blend,
            curve,
            duration_sec,
            lock_input: false,
            preserve_previous_frame: true,
        }
    }

    #[inline]
    pub const fn with_lock_input(mut self, lock_input: bool) -> Self {
        self.lock_input = lock_input;
        self
    }

    #[inline]
    pub fn sample(&self, elapsed_sec: f32) -> f32 {
        if self.policy == CameraFrameBlendPolicy::Cut || self.duration_sec <= 0.0 {
            return 1.0;
        }
        let t = (elapsed_sec / self.duration_sec.max(1.0e-6)).clamp(0.0, 1.0);
        match self.curve {
            CameraFrameBlendCurve::Linear => t,
            CameraFrameBlendCurve::SmoothStep => t * t * (3.0 - 2.0 * t),
            CameraFrameBlendCurve::EaseInOut => {
                if t < 0.5 {
                    2.0 * t * t
                } else {
                    1.0 - (-2.0 * t + 2.0).powi(2) * 0.5
                }
            }
        }
    }
}

impl Default for CameraFrameBlendPlan {
    #[inline]
    fn default() -> Self {
        Self::cut()
    }
}

/// Runtime state for an in-flight camera-frame blend.
///
/// `last_output` is the actually presented frame from the previous tick. When a new blend starts,
/// it becomes the immutable `from_frame`, while each new mode/director tick supplies the target
/// frame. This avoids progressive self-blending and keeps transitions deterministic.
#[derive(Clone, Copy, Debug)]
pub struct CameraFrameBlendState {
    pub plan: CameraFrameBlendPlan,
    pub elapsed_sec: f32,
    pub active: bool,
    pub from_frame: Option<CameraFrame>,
    pub last_output: Option<CameraFrame>,
}

impl Default for CameraFrameBlendState {
    #[inline]
    fn default() -> Self {
        Self {
            plan: CameraFrameBlendPlan::cut(),
            elapsed_sec: 0.0,
            active: false,
            from_frame: None,
            last_output: None,
        }
    }
}

impl CameraFrameBlendState {
    #[inline]
    pub fn begin(&mut self, plan: CameraFrameBlendPlan) {
        self.plan = plan;
        self.elapsed_sec = 0.0;
        self.active = plan.policy == CameraFrameBlendPolicy::Blend && plan.duration_sec > 0.0;
        self.from_frame = if self.active && plan.preserve_previous_frame {
            self.last_output
        } else {
            None
        };
    }

    #[inline]
    pub fn is_active(&self) -> bool {
        self.active
    }

    #[inline]
    pub fn alpha(&self) -> f32 {
        if !self.active {
            1.0
        } else {
            self.plan.sample(self.elapsed_sec)
        }
    }

    #[inline]
    pub fn resolve(&mut self, target: CameraFrame, dt: f32) -> CameraFrame {
        if !self.active {
            self.last_output = Some(target);
            return target;
        }

        if dt.is_finite() && dt > 0.0 {
            self.elapsed_sec += dt;
        }

        let alpha = self.plan.sample(self.elapsed_sec);
        let output = match self.from_frame {
            Some(from) if alpha < 1.0 => blend_camera_frames(from, target, alpha),
            _ => target,
        };

        if alpha >= 1.0 {
            self.active = false;
            self.from_frame = None;
            self.plan = CameraFrameBlendPlan::cut();
            self.elapsed_sec = 0.0;
            self.last_output = Some(target);
            target
        } else {
            self.last_output = Some(output);
            output
        }
    }
}

#[inline]
pub fn blend_camera_frames(from: CameraFrame, to: CameraFrame, t: f32) -> CameraFrame {
    let t = t.clamp(0.0, 1.0);
    let rig = CameraRig {
        position: lerp_vec3(from.rig.position, to.rig.position, t),
        rotation: from.rig.rotation.slerp(to.rig.rotation, t).normalize_or_identity(),
    };
    let projection = blend_projection(from.projection, to.projection, t);
    let jitter = lerp_vec2(from.jitter_px, to.jitter_px, t);

    CameraFrame::build(to.channel, rig, projection, to.viewport, jitter)
}

#[inline]
fn blend_projection(from: Projection, to: Projection, t: f32) -> Projection {
    match (from, to) {
        (Projection::Perspective(a), Projection::Perspective(b)) => {
            Projection::Perspective(Perspective::new(
                lerp_f32(a.fovy, b.fovy, t),
                lerp_f32(a.aspect, b.aspect, t),
                lerp_f32(a.near, b.near, t).max(1.0e-6),
                lerp_f32(a.far, b.far, t).max(1.0e-3),
            ))
        }
        (Projection::Orthographic(a), Projection::Orthographic(b)) => {
            Projection::Orthographic(Orthographic::new(
                lerp_f32(a.half_height, b.half_height, t).max(1.0e-6),
                lerp_f32(a.aspect, b.aspect, t),
                lerp_f32(a.near, b.near, t).max(1.0e-6),
                lerp_f32(a.far, b.far, t).max(1.0e-3),
            ))
        }
        _ => {
            if t < 0.5 {
                from
            } else {
                to
            }
        }
    }
}

#[inline]
fn lerp_vec3(a: Vec3, b: Vec3, t: f32) -> Vec3 {
    a + (b - a) * t
}

#[inline]
fn lerp_vec2(a: Vec2, b: Vec2, t: f32) -> Vec2 {
    a + (b - a) * t
}

#[inline]
fn lerp_f32(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}
