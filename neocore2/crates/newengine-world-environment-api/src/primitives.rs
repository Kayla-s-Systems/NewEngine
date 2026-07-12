use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Vec3Dto {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vec3Dto {
    #[inline]
    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    #[inline]
    pub const fn zero() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        }
    }

    #[inline]
    pub const fn up() -> Self {
        Self {
            x: 0.0,
            y: 1.0,
            z: 0.0,
        }
    }
}

impl Default for Vec3Dto {
    #[inline]
    fn default() -> Self {
        Self::zero()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Color3Dto {
    pub r: f32,
    pub g: f32,
    pub b: f32,
}

impl Color3Dto {
    #[inline]
    pub const fn new(r: f32, g: f32, b: f32) -> Self {
        Self { r, g, b }
    }

    #[inline]
    pub const fn black() -> Self {
        Self {
            r: 0.0,
            g: 0.0,
            b: 0.0,
        }
    }

    #[inline]
    pub const fn white() -> Self {
        Self {
            r: 1.0,
            g: 1.0,
            b: 1.0,
        }
    }
}

impl Default for Color3Dto {
    #[inline]
    fn default() -> Self {
        Self::black()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AabbDto {
    pub min: Vec3Dto,
    pub max: Vec3Dto,
}

impl Default for AabbDto {
    #[inline]
    fn default() -> Self {
        Self {
            min: Vec3Dto::zero(),
            max: Vec3Dto::zero(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TransformDto {
    pub translation: Vec3Dto,
    pub rotation_quat_xyzw: [f32; 4],
    pub scale: Vec3Dto,
}

impl Default for TransformDto {
    #[inline]
    fn default() -> Self {
        Self {
            translation: Vec3Dto::zero(),
            rotation_quat_xyzw: [0.0, 0.0, 0.0, 1.0],
            scale: Vec3Dto::new(1.0, 1.0, 1.0),
        }
    }
}
