#![forbid(unsafe_op_in_unsafe_fn)]

use crate::{builtins, PrimitiveId};

/// Renderable primitive component.
///
/// Concrete renderers are expected to map `id` into GPU meshes,
/// usually via a `PrimitiveRegistry` or pre-baked mesh cache.
#[derive(Clone, Copy, Debug)]
pub struct Primitive {
    pub id: PrimitiveId,
    pub color: [f32; 4],
}

impl Default for Primitive {
    #[inline]
    fn default() -> Self {
        Self {
            id: builtins::ID_CUBE,
            color: [0.85, 0.85, 0.9, 1.0],
        }
    }
}