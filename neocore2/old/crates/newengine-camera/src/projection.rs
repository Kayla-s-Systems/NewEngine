#![forbid(unsafe_op_in_unsafe_fn)]

use glam::Mat4;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// Projection model for the camera.
///
/// Important: `set_viewport()` must be called on resize to update aspect ratio.
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum Projection {
    Perspective(Perspective),
    Orthographic(Orthographic),
}

impl Projection {
    #[inline]
    pub fn set_viewport(&mut self, width: u32, height: u32) {
        let w = width.max(1) as f32;
        let h = height.max(1) as f32;
        let aspect = w / h;
        match self {
            Self::Perspective(p) => p.aspect = aspect,
            Self::Orthographic(o) => o.aspect = aspect,
        }
    }

    #[inline]
    pub fn near_far(&self) -> (f32, f32) {
        match self {
            Self::Perspective(p) => (p.near, p.far),
            Self::Orthographic(o) => (o.near, o.far),
        }
    }

    #[inline]
    pub fn matrix(&self) -> Mat4 {
        match self {
            Self::Perspective(p) => p.matrix(),
            Self::Orthographic(o) => o.matrix(),
        }
    }
}

#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Perspective {
    /// Vertical FOV in radians.
    pub fovy: f32,
    pub aspect: f32,
    pub near: f32,
    pub far: f32,
}

impl Perspective {
    #[inline]
    pub fn new(fovy: f32, aspect: f32, near: f32, far: f32) -> Self {
        Self {
            fovy,
            aspect: aspect.max(1e-6),
            near: near.max(1e-6),
            far: far.max(near + 1e-3),
        }
    }

    #[inline]
    pub fn matrix(&self) -> Mat4 {
        Mat4::perspective_rh(self.fovy, self.aspect, self.near, self.far)
    }
}

#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Orthographic {
    /// Half-height in world units. Width is derived from aspect.
    pub half_height: f32,
    pub aspect: f32,
    pub near: f32,
    pub far: f32,
}

impl Orthographic {
    #[inline]
    pub fn new(half_height: f32, aspect: f32, near: f32, far: f32) -> Self {
        Self {
            half_height: half_height.max(1e-6),
            aspect: aspect.max(1e-6),
            near: near.max(1e-6),
            far: far.max(near + 1e-3),
        }
    }

    #[inline]
    pub fn matrix(&self) -> Mat4 {
        let hh = self.half_height;
        let hw = hh * self.aspect;
        Mat4::orthographic_rh(-hw, hw, -hh, hh, self.near, self.far)
    }
}