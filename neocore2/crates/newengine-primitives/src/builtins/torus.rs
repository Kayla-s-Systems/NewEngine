#![forbid(unsafe_op_in_unsafe_fn)]

use core::f32::consts::PI;

use newengine_math::Vec3;

use crate::registry::PrimitiveParams;
use crate::{PrimitiveMesh, PrimitiveVertex};

#[inline]
fn clamp_u32(v: u32, lo: u32, hi: u32) -> u32 {
    v.max(lo).min(hi)
}

/// Unit torus centered at origin.
///
/// Defaults:
/// - major radius = 0.35
/// - minor radius = 0.15
///
/// Params:
/// - `major_segments` (default 48)
/// - `minor_segments` (default 16)
#[inline]
pub fn build(params: &PrimitiveParams) -> PrimitiveMesh {
    let maj = clamp_u32(params.major_segments, 3, 4096);
    let min = clamp_u32(params.minor_segments, 3, 4096);

    let major_r = 0.35f32;
    let minor_r = 0.15f32;

    let vert_w = maj + 1;
    let vert_h = min + 1;
    let vtx_count = (vert_w * vert_h) as usize;
    let tri_count = (maj * min * 2) as usize;

    let mut vertices = Vec::with_capacity(vtx_count);
    for j in 0..=min {
        let v = (j as f32) * (2.0 * PI) / (min as f32);
        let (sv, cv) = v.sin_cos();
        for i in 0..=maj {
            let u = (i as f32) * (2.0 * PI) / (maj as f32);
            let (su, cu) = u.sin_cos();

            let x = (major_r + minor_r * cv) * cu;
            let y = minor_r * sv;
            let z = (major_r + minor_r * cv) * su;

            let n0 = Vec3::new(cu * cv, sv, su * cv);
            let n = n0 * (1.0 / n0.length().max(1.0e-20));

            vertices.push(PrimitiveVertex {
                pos: [x, y, z],
                nrm: [n.x, n.y, n.z],
            });
        }
    }

    let mut indices = Vec::with_capacity(tri_count * 3);
    for j in 0..min {
        for i in 0..maj {
            let i0 = j * vert_w + i;
            let i1 = i0 + 1;
            let i2 = i0 + vert_w;
            let i3 = i2 + 1;
            indices.extend_from_slice(&[i0, i2, i1, i1, i2, i3]);
        }
    }

    PrimitiveMesh {
        vertices,
        indices,
        bounds_center: Vec3::ZERO,
        bounds_radius: major_r + minor_r,
    }
}
