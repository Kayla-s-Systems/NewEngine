#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_math::Mat4;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// Camera projection.
///
/// Engine baseline:
/// - Right-handed.
/// - Vulkan clip Z: 0..1.
/// - Y flip baked into the matrix.
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

    /// Vulkan-ready projection matrix.
    #[inline]
    pub fn matrix(&self) -> Mat4 {
        match self {
            Self::Perspective(p) => p.matrix_vk(),
            Self::Orthographic(o) => o.matrix_vk(),
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

    /// RH perspective, Vulkan Z: 0..1, Y flipped.
    #[inline]
    pub fn matrix_vk(&self) -> Mat4 {
        let f = 1.0 / (0.5 * self.fovy).tan();
        let aspect = self.aspect.max(1e-6);
        let near = self.near.max(1e-6);
        let far = self.far.max(near + 1e-3);
        let nf = 1.0 / (near - far);

        Mat4::from_cols_array(&[
            f / aspect, 0.0, 0.0, 0.0,
            0.0, -f, 0.0, 0.0,
            0.0, 0.0, far * nf, -1.0,
            0.0, 0.0, far * near * nf, 0.0,
        ])
    }

    /// GL-style RH perspective, Z: -1..1, no Y flip.
    #[inline]
    pub fn matrix_gl(&self) -> Mat4 {
        Mat4::perspective_rh(self.fovy, self.aspect, self.near, self.far)
    }
}

#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Orthographic {
    /// Half-height in world units.
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

    /// RH orthographic, Vulkan Z: 0..1, Y flipped.
    #[inline]
    pub fn matrix_vk(&self) -> Mat4 {
        let hh = self.half_height.max(1e-6);
        let hw = hh * self.aspect.max(1e-6);
        let near = self.near.max(1e-6);
        let far = self.far.max(near + 1e-3);

        let m00 = 1.0 / hw;
        let m11 = -1.0 / hh;
        let m22 = 1.0 / (near - far);
        let m32 = near / (near - far);

        Mat4::from_cols_array(&[
            m00, 0.0, 0.0, 0.0,
            0.0, m11, 0.0, 0.0,
            0.0, 0.0, m22, 0.0,
            0.0, 0.0, m32, 1.0,
        ])
    }

    /// GL-style RH orthographic, Z: -1..1, no Y flip.
    #[inline]
    pub fn matrix_gl(&self) -> Mat4 {
        let hh = self.half_height;
        let hw = hh * self.aspect;
        Mat4::orthographic_rh(-hw, hw, -hh, hh, self.near, self.far)
    }
}
