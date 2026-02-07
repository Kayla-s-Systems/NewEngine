#![allow(clippy::excessive_precision)]

use bytemuck::{Pod, Zeroable};

#[cfg(feature = "abi")]
use abi_stable::StableAbi;

#[repr(C)]
#[cfg_attr(feature = "abi", derive(StableAbi))]
#[derive(Clone, Copy, Default, Debug, PartialEq, Zeroable, Pod)]
pub struct Vec3f {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vec3f {
    #[inline]
    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    #[inline]
    pub const fn zero() -> Self {
        Self::new(0.0, 0.0, 0.0)
    }
}

#[repr(C)]
#[cfg_attr(feature = "abi", derive(StableAbi))]
#[derive(Clone, Copy, Default, Debug, PartialEq, Zeroable, Pod)]
pub struct Quatf {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub w: f32,
}

impl Quatf {
    #[inline]
    pub const fn identity() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            z: 0.0,
            w: 1.0,
        }
    }
}
