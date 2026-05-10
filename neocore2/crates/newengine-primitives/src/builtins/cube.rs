#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_math::Vec3;

use crate::registry::PrimitiveParams;
use crate::{PrimitiveMesh, PrimitiveVertex};

#[inline]
pub fn build(_params: &PrimitiveParams) -> PrimitiveMesh {
    let p = |x: f32, y: f32, z: f32, nx: f32, ny: f32, nz: f32, u: f32, v: f32| PrimitiveVertex {
        pos: [x, y, z],
        nrm: [nx, ny, nz],
        uv: [u, v],
    };

    let h = 0.5f32;
    let mut v: Vec<PrimitiveVertex> = Vec::with_capacity(24);
    v.extend_from_slice(&[
        p(-h, -h, h, 0.0, 0.0, 1.0, 0.0, 0.0),
        p(h, -h, h, 0.0, 0.0, 1.0, 1.0, 0.0),
        p(h, h, h, 0.0, 0.0, 1.0, 1.0, 1.0),
        p(-h, h, h, 0.0, 0.0, 1.0, 0.0, 1.0),
        p(h, -h, -h, 0.0, 0.0, -1.0, 0.0, 0.0),
        p(-h, -h, -h, 0.0, 0.0, -1.0, 1.0, 0.0),
        p(-h, h, -h, 0.0, 0.0, -1.0, 1.0, 1.0),
        p(h, h, -h, 0.0, 0.0, -1.0, 0.0, 1.0),
        p(h, -h, h, 1.0, 0.0, 0.0, 0.0, 0.0),
        p(h, -h, -h, 1.0, 0.0, 0.0, 1.0, 0.0),
        p(h, h, -h, 1.0, 0.0, 0.0, 1.0, 1.0),
        p(h, h, h, 1.0, 0.0, 0.0, 0.0, 1.0),
        p(-h, -h, -h, -1.0, 0.0, 0.0, 0.0, 0.0),
        p(-h, -h, h, -1.0, 0.0, 0.0, 1.0, 0.0),
        p(-h, h, h, -1.0, 0.0, 0.0, 1.0, 1.0),
        p(-h, h, -h, -1.0, 0.0, 0.0, 0.0, 1.0),
        p(-h, h, h, 0.0, 1.0, 0.0, 0.0, 0.0),
        p(h, h, h, 0.0, 1.0, 0.0, 1.0, 0.0),
        p(h, h, -h, 0.0, 1.0, 0.0, 1.0, 1.0),
        p(-h, h, -h, 0.0, 1.0, 0.0, 0.0, 1.0),
        p(-h, -h, -h, 0.0, -1.0, 0.0, 0.0, 0.0),
        p(h, -h, -h, 0.0, -1.0, 0.0, 1.0, 0.0),
        p(h, -h, h, 0.0, -1.0, 0.0, 1.0, 1.0),
        p(-h, -h, h, 0.0, -1.0, 0.0, 0.0, 1.0),
    ]);

    let mut i: Vec<u32> = Vec::with_capacity(36);
    for f in 0..6u32 {
        let base = f * 4;
        i.extend_from_slice(&[base + 0, base + 1, base + 2, base + 0, base + 2, base + 3]);
    }

    PrimitiveMesh {
        vertices: v,
        indices: i,
        bounds_center: Vec3::ZERO,
        bounds_radius: Vec3::splat(h).length(),
    }
}
