#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_math::Vec3;

use crate::registry::PrimitiveParams;
use crate::{PrimitiveMesh, PrimitiveVertex};

#[inline]
fn clamp_u32(v: u32, lo: u32, hi: u32) -> u32 {
    v.max(lo).min(hi)
}

#[inline]
pub fn build(params: &PrimitiveParams) -> PrimitiveMesh {
    let n = clamp_u32(params.subdivisions, 1, 2048);
    let h = 0.5f32;
    let step = 1.0f32 / (n as f32);

    let w = n + 1;
    let mut vertices = Vec::with_capacity((w * w) as usize);
    for z in 0..=n {
        let fz = (z as f32) * step;
        let pz = -h + fz;
        for x in 0..=n {
            let fx = (x as f32) * step;
            let px = -h + fx;
            vertices.push(PrimitiveVertex {
                pos: [px, 0.0, pz],
                nrm: [0.0, 1.0, 0.0],
                uv: [fx, fz],
            });
        }
    }

    let mut indices = Vec::with_capacity((n * n * 2) as usize * 3);
    for z in 0..n {
        for x in 0..n {
            let i0 = z * w + x;
            let i1 = i0 + 1;
            let i2 = i0 + w;
            let i3 = i2 + 1;
            indices.extend_from_slice(&[i0, i2, i1, i1, i2, i3]);
        }
    }

    PrimitiveMesh {
        vertices,
        indices,
        bounds_center: Vec3::ZERO,
        bounds_radius: Vec3::new(h, 0.0, h).length(),
    }
}
