#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_math::{Vec2, Vec4};

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// Renderer-independent viewport rectangle in physical pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct CameraViewport {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl Default for CameraViewport {
    #[inline]
    fn default() -> Self {
        Self::new(0, 0, 1920, 1080)
    }
}

impl CameraViewport {
    #[inline]
    pub const fn new(x: u32, y: u32, width: u32, height: u32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    #[inline]
    pub fn from_size(width: u32, height: u32) -> Self {
        Self::new(0, 0, width.max(1), height.max(1))
    }

    #[inline]
    pub fn sanitized(self) -> Self {
        Self {
            width: self.width.max(1),
            height: self.height.max(1),
            ..self
        }
    }

    #[inline]
    pub fn aspect(self) -> f32 {
        let vp = self.sanitized();
        (vp.width as f32 / vp.height as f32).max(1.0e-6)
    }

    #[inline]
    pub fn size_vec2(self) -> Vec2 {
        let vp = self.sanitized();
        Vec2::new(vp.width as f32, vp.height as f32)
    }

    #[inline]
    pub fn uniform(self) -> Vec4 {
        let vp = self.sanitized();
        let w = vp.width as f32;
        let h = vp.height as f32;
        Vec4::new(w, h, 1.0 / w, 1.0 / h)
    }
}
