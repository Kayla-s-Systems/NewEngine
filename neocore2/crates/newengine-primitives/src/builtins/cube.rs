#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_math::Vec3;

use crate::registry::PrimitiveParams;
use crate::{PrimitiveMesh, PrimitiveVertex};

#[inline]
pub fn build(_params: &PrimitiveParams) -> PrimitiveMesh {
    // Unit cube centered at origin. 24 vertices (4 per face) for hard normals.
    let p = |x: f32, y: f32, z: f32, nx: f32, ny: f32, nz: f32| PrimitiveVertex {
        pos: [x, y, z],
        nrm: [nx, ny, nz],
    };

    let h = 0.5f32;

    let mut v: Vec<PrimitiveVertex> = Vec::with_capacity(24);

    // +Z
    v.extend_from_slice(&[
        p(-h, -h, h, 0.0, 0.0, 1.0),
        p(h, -h, h, 0.0, 0.0, 1.0),
        p(h, h, h, 0.0, 0.0, 1.0),
        p(-h, h, h, 0.0, 0.0, 1.0),
    ]);
    // -Z
    v.extend_from_slice(&[
        p(h, -h, -h, 0.0, 0.0, -1.0),
        p(-h, -h, -h, 0.0, 0.0, -1.0),
        p(-h, h, -h, 0.0, 0.0, -1.0),
        p(h, h, -h, 0.0, 0.0, -1.0),
    ]);
    // +X
    v.extend_from_slice(&[
        p(h, -h, h, 1.0, 0.0, 0.0),
        p(h, -h, -h, 1.0, 0.0, 0.0),
        p(h, h, -h, 1.0, 0.0, 0.0),
        p(h, h, h, 1.0, 0.0, 0.0),
    ]);
    // -X
    v.extend_from_slice(&[
        p(-h, -h, -h, -1.0, 0.0, 0.0),
        p(-h, -h, h, -1.0, 0.0, 0.0),
        p(-h, h, h, -1.0, 0.0, 0.0),
        p(-h, h, -h, -1.0, 0.0, 0.0),
    ]);
    // +Y
    v.extend_from_slice(&[
        p(-h, h, h, 0.0, 1.0, 0.0),
        p(h, h, h, 0.0, 1.0, 0.0),
        p(h, h, -h, 0.0, 1.0, 0.0),
        p(-h, h, -h, 0.0, 1.0, 0.0),
    ]);
    // -Y
    v.extend_from_slice(&[
        p(-h, -h, -h, 0.0, -1.0, 0.0),
        p(h, -h, -h, 0.0, -1.0, 0.0),
        p(h, -h, h, 0.0, -1.0, 0.0),
        p(-h, -h, h, 0.0, -1.0, 0.0),
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
