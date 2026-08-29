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
#[derive(Clone, Copy, Debug, PartialEq)]
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
    pub fn set_near_far(&mut self, near: f32, far: f32) {
        let near = if near.is_finite() && near > 0.0 {
            near
        } else {
            0.01
        };
        let far = if far.is_finite() && far > near {
            far
        } else {
            near + 1000.0
        };
        match self {
            Self::Perspective(p) => {
                p.near = near;
                p.far = far.max(near + 0.001);
            }
            Self::Orthographic(o) => {
                o.near = near;
                o.far = far.max(near + 0.001);
            }
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

#[derive(Clone, Copy, Debug, PartialEq)]
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
        let fovy = if fovy.is_finite() {
            fovy.clamp(1.0_f32.to_radians(), 170.0_f32.to_radians())
        } else {
            60.0_f32.to_radians()
        };
        let aspect = if aspect.is_finite() && aspect > 0.0 {
            aspect
        } else {
            16.0 / 9.0
        };
        let near = if near.is_finite() && near > 0.0 {
            near
        } else {
            0.01
        };
        let far = if far.is_finite() && far > near {
            far
        } else {
            near + 1000.0
        };
        Self {
            fovy,
            aspect: aspect.max(1e-6),
            near,
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
            f / aspect,
            0.0,
            0.0,
            0.0,
            0.0,
            -f,
            0.0,
            0.0,
            0.0,
            0.0,
            far * nf,
            -1.0,
            0.0,
            0.0,
            far * near * nf,
            0.0,
        ])
    }

    /// RH perspective, Vulkan Z: reversed 1..0, Y flipped.
    ///
    /// This is useful when the render backend uses a reversed-Z
    /// depth buffer. It is opt-in because the backend depth compare state must match it.
    #[inline]
    pub fn matrix_vk_reversed_z(&self) -> Mat4 {
        let f = 1.0 / (0.5 * self.fovy).tan();
        let aspect = self.aspect.max(1e-6);
        let near = self.near.max(1e-6);
        let far = self.far.max(near + 1e-3);
        let inv = 1.0 / (far - near);

        Mat4::from_cols_array(&[
            f / aspect,
            0.0,
            0.0,
            0.0,
            0.0,
            -f,
            0.0,
            0.0,
            0.0,
            0.0,
            near * inv,
            -1.0,
            0.0,
            0.0,
            far * near * inv,
            0.0,
        ])
    }

    /// RH infinite-far perspective, Vulkan reversed-Z, Y flipped.
    ///
    /// Use only with a reversed-Z depth state. This avoids far-plane precision collapse for
    /// streamed worlds while keeping the public `Projection` shape unchanged.
    #[inline]
    pub fn matrix_vk_reversed_z_infinite_far(&self) -> Mat4 {
        let f = 1.0 / (0.5 * self.fovy).tan();
        let aspect = self.aspect.max(1e-6);
        let near = self.near.max(1e-6);

        Mat4::from_cols_array(&[
            f / aspect,
            0.0,
            0.0,
            0.0,
            0.0,
            -f,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            -1.0,
            0.0,
            0.0,
            near,
            0.0,
        ])
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
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
        let half_height = if half_height.is_finite() && half_height > 0.0 {
            half_height
        } else {
            1.0
        };
        let aspect = if aspect.is_finite() && aspect > 0.0 {
            aspect
        } else {
            16.0 / 9.0
        };
        let near = if near.is_finite() && near > 0.0 {
            near
        } else {
            0.01
        };
        let far = if far.is_finite() && far > near {
            far
        } else {
            near + 1000.0
        };
        Self {
            half_height: half_height.max(1e-6),
            aspect: aspect.max(1e-6),
            near,
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
            m00, 0.0, 0.0, 0.0, 0.0, m11, 0.0, 0.0, 0.0, 0.0, m22, 0.0, 0.0, 0.0, m32, 1.0,
        ])
    }
}

/// Blends two camera projections using a stable same-kind interpolation.
///
/// Different projection families cut at the half-way mark because interpolating between
/// perspective and orthographic matrices directly produces poor gameplay/cinematic semantics.
#[inline]
pub fn blend_camera_projection(from: Projection, to: Projection, t: f32) -> Projection {
    let t = t.clamp(0.0, 1.0);
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
fn lerp_f32(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}
