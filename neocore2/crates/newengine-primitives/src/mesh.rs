#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_math::Vec3;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use crate::PrimitiveVertex;

/// Deterministic CPU mesh.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct PrimitiveMesh {
    pub vertices: Vec<PrimitiveVertex>,
    pub indices: Vec<u32>,
    pub bounds_center: Vec3,
    pub bounds_radius: f32,
}
