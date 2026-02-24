// Copyright (c) 2026 NewEngine | Kayla's Systems. All rights reserved.
#![forbid(unsafe_op_in_unsafe_fn)]

use crate::{Vec2, Vec3};

/// Extension methods for [`Vec2`] and [`Vec3`].
///
/// This exists to provide stable, engine-level naming across potential math backends.
/// Keep call sites stable.
pub trait Vec2Ext {
    /// Returns squared length of the vector.
    fn length_sq(self) -> f32;

    /// Returns squared distance between two vectors.
    fn distance_sq(self, other: Vec2) -> f32;
}

impl Vec2Ext for Vec2 {
    #[inline]
    fn length_sq(self) -> f32 {
        self.length_squared()
    }
    #[inline]
    fn distance_sq(self, other: Vec2) -> f32 {
        self.distance_squared(other)
    }
}

/// Extension methods for [`Vec3`].
pub trait Vec3Ext {
    /// Returns squared length of the vector.
    fn length_sq(self) -> f32;

    /// Returns squared distance between two vectors.
    fn distance_sq(self, other: Vec3) -> f32;
}

impl Vec3Ext for Vec3 {
    #[inline]
    fn length_sq(self) -> f32 {
        self.length_squared()
    }
    #[inline]
    fn distance_sq(self, other: Vec3) -> f32 {
        self.distance_squared(other)
    }
}
