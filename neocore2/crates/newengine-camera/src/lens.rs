#![forbid(unsafe_op_in_unsafe_fn)]

use crate::{Orthographic, Perspective, Projection};

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// Runtime lens/clip policy.
///
/// Keep the lens as explicit data so gameplay, cinematic, editor and replay cameras can use the
/// same simple builder while still exposing all values to diagnostics.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct CameraLens {
    pub fovy: f32,
    pub near: f32,
    pub far: f32,
    pub min_near: f32,
    pub max_far: f32,
}

impl Default for CameraLens {
    #[inline]
    fn default() -> Self {
        Self::perspective_60()
    }
}

impl CameraLens {
    #[inline]
    pub const fn new(fovy: f32, near: f32, far: f32) -> Self {
        Self {
            fovy,
            near,
            far,
            min_near: 0.01,
            max_far: 100_000.0,
        }
    }

    #[inline]
    pub const fn perspective_60() -> Self {
        Self::new(core::f32::consts::FRAC_PI_3, 0.01, 10_000.0)
    }

    #[inline]
    pub fn with_clip(mut self, near: f32, far: f32) -> Self {
        self.near = near;
        self.far = far;
        self.sanitized()
    }

    #[inline]
    pub fn with_far_limit(mut self, max_far: f32) -> Self {
        if max_far.is_finite() && max_far > self.min_near {
            self.max_far = max_far;
        }
        self.sanitized()
    }

    #[inline]
    pub fn sanitized(self) -> Self {
        let fovy = if self.fovy.is_finite() {
            self.fovy
                .clamp(1.0_f32.to_radians(), 170.0_f32.to_radians())
        } else {
            60.0_f32.to_radians()
        };
        let min_near = finite_positive_or(self.min_near, 0.01).max(0.0005);
        let max_far = finite_positive_or(self.max_far, 100_000.0).max(min_near + 1.0);
        let near = finite_positive_or(self.near, min_near)
            .max(min_near)
            .min(max_far - 0.001);
        let far = finite_positive_or(self.far, max_far.min(10_000.0))
            .max(near + 0.001)
            .min(max_far);
        Self {
            fovy,
            near,
            far,
            min_near,
            max_far,
        }
    }

    #[inline]
    pub fn projection(self, aspect: f32) -> Projection {
        let lens = self.sanitized();
        Projection::Perspective(Perspective::new(
            lens.fovy,
            aspect.max(1.0e-6),
            lens.near,
            lens.far,
        ))
    }

    #[inline]
    pub fn projection_for_focus(self, aspect: f32, distance: f32, radius: f32) -> Projection {
        let mut lens = self.sanitized();
        let (near, far) = CameraClipPolicy::default().near_far(distance, radius, lens.max_far);
        lens.near = near;
        lens.far = far;
        lens.projection(aspect)
    }
}

/// Orthographic lens helper used by editor/replay/top-down modes.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct CameraOrthoLens {
    pub half_height: f32,
    pub near: f32,
    pub far: f32,
}

impl CameraOrthoLens {
    #[inline]
    pub const fn new(half_height: f32, near: f32, far: f32) -> Self {
        Self {
            half_height,
            near,
            far,
        }
    }

    #[inline]
    pub fn projection(self, aspect: f32) -> Projection {
        Projection::Orthographic(Orthographic::new(
            finite_positive_or(self.half_height, 1.0),
            aspect.max(1.0e-6),
            finite_positive_or(self.near, 0.01),
            finite_positive_or(self.far, 10_000.0),
        ))
    }
}

/// Heuristic for robust near/far planes.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct CameraClipPolicy {
    pub min_near: f32,
    pub near_distance_ratio: f32,
    pub near_radius_ratio: f32,
    pub far_radius_margin: f32,
    pub min_span: f32,
}

impl Default for CameraClipPolicy {
    #[inline]
    fn default() -> Self {
        Self {
            min_near: 0.01,
            near_distance_ratio: 0.001,
            near_radius_ratio: 0.02,
            far_radius_margin: 4.0,
            min_span: 100.0,
        }
    }
}

impl CameraClipPolicy {
    #[inline]
    pub fn world_space() -> Self {
        Self {
            min_near: 0.03,
            near_distance_ratio: 0.0005,
            near_radius_ratio: 0.01,
            far_radius_margin: 3.0,
            min_span: 250.0,
        }
    }

    #[inline]
    pub fn near_far(self, distance: f32, radius: f32, max_far: f32) -> (f32, f32) {
        let d = finite_positive_or(distance.abs(), 1.0).max(0.01);
        let r = finite_positive_or(radius.abs(), 1.0).max(0.001);
        let max_far = finite_positive_or(max_far, 100_000.0).max(self.min_span.max(1.0));

        let near_by_distance = d * self.near_distance_ratio.max(0.0);
        let near_by_radius = r * self.near_radius_ratio.max(0.0);
        let near_by_front = (d - r * 1.25).max(self.min_near);
        let near = near_by_distance
            .max(near_by_radius)
            .min(near_by_front)
            .max(self.min_near)
            .min(max_far - 0.001);

        let far = (d + r * self.far_radius_margin.max(1.0))
            .max(near + self.min_span.max(1.0))
            .min(max_far)
            .max(near + 0.001);
        (near, far)
    }
}

#[inline]
fn finite_positive_or(value: f32, fallback: f32) -> f32 {
    if value.is_finite() && value > 0.0 {
        value
    } else {
        fallback
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clip_policy_never_inverts_planes() {
        let (near, far) = CameraClipPolicy::world_space().near_far(1_000_000.0, 10.0, 100_000.0);
        assert!(near > 0.0);
        assert!(far > near);
        assert!(far <= 100_000.0);
    }

    #[test]
    fn lens_sanitizes_invalid_values() {
        let lens = CameraLens::new(f32::NAN, -1.0, -5.0).sanitized();
        assert!(lens.fovy.is_finite());
        assert!(lens.near > 0.0);
        assert!(lens.far > lens.near);
    }
}
