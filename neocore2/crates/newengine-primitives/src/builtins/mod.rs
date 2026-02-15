#![forbid(unsafe_op_in_unsafe_fn)]

mod cube;
mod plane;

use crate::{fnv1a_64, PrimitiveId, PrimitiveRegistry};

pub const ID_CUBE: PrimitiveId = PrimitiveId(fnv1a_64("kalitech.primitive.cube.v1"));
pub const ID_PLANE: PrimitiveId = PrimitiveId(fnv1a_64("kalitech.primitive.plane.v1"));

#[inline]
pub fn register(r: &mut PrimitiveRegistry) {
    r.register(ID_CUBE, "Cube", cube::build);
    r.register(ID_PLANE, "Plane", plane::build);
}