#![forbid(unsafe_op_in_unsafe_fn)]

use bytemuck::{Pod, Zeroable};

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// Standard vertex format for primitives.
///
/// Layout:
/// - location 0: position (vec3)
/// - location 1: normal (vec3)
/// - location 2: uv (vec2)
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Zeroable, Pod)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct PrimitiveVertex {
    pub pos: [f32; 3],
    pub nrm: [f32; 3],
    pub uv: [f32; 2],
}
