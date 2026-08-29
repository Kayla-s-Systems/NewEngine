#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_math::Vec3;

use crate::registry::PrimitiveParams;
use crate::{PrimitiveMesh, PrimitiveVertex};

#[inline]
pub fn build(_params: &PrimitiveParams) -> PrimitiveMesh {
    let h = 0.5f32;

    let v = vec![
        PrimitiveVertex {
            pos: [-h, 0.0, -h],
            nrm: [0.0, 1.0, 0.0],
            uv: [0.0, 0.0],
        },
        PrimitiveVertex {
            pos: [h, 0.0, -h],
            nrm: [0.0, 1.0, 0.0],
            uv: [1.0, 0.0],
        },
        PrimitiveVertex {
            pos: [h, 0.0, h],
            nrm: [0.0, 1.0, 0.0],
            uv: [1.0, 1.0],
        },
        PrimitiveVertex {
            pos: [-h, 0.0, h],
            nrm: [0.0, 1.0, 0.0],
            uv: [0.0, 1.0],
        },
    ];

    let i = vec![0u32, 1, 2, 0, 2, 3];

    PrimitiveMesh {
        vertices: v,
        indices: i,
        bounds_center: Vec3::ZERO,
        bounds_radius: Vec3::new(h, 0.0, h).length(),
    }
}
