#![forbid(unsafe_op_in_unsafe_fn)]

use core::f32::consts::PI;

use newengine_math::Vec3;

use crate::registry::PrimitiveParams;
use crate::{PrimitiveMesh, PrimitiveVertex};

#[inline]
fn clamp_u32(v: u32, lo: u32, hi: u32) -> u32 {
    v.max(lo).min(hi)
}

/// Unit cone centered at origin.
///
/// - base radius = 0.5 at y = -0.5
/// - apex at y = +0.5
/// - axis = Y
///
/// Params:
/// - `segments` (default 32)
#[inline]
pub fn build(params: &PrimitiveParams) -> PrimitiveMesh {
    let seg = clamp_u32(params.segments, 3, 4096);
    let r = 0.5f32;
    let hy = 0.5f32;

    // side ring (seg+1) + apex duplicated per segment via indexing
    let mut vertices = Vec::with_capacity(((seg + 1) + 1 + (seg + 1) + 1) as usize);

    // side vertices (base ring)
    let slope = r / (2.0 * hy); // r / height
    for i in 0..=seg {
        let a = (i as f32) * (2.0 * PI) / (seg as f32);
        let (sa, ca) = a.sin_cos();
        let n0 = Vec3::new(ca, slope, sa);
        let n = n0 * (1.0 / n0.length().max(1.0e-20));
        vertices.push(PrimitiveVertex {
            pos: [ca * r, -hy, sa * r],
            nrm: [n.x, n.y, n.z],
        });
    }

    let apex_index = vertices.len() as u32;
    vertices.push(PrimitiveVertex {
        pos: [0.0, hy, 0.0],
        nrm: [0.0, 1.0, 0.0],
    });

    // base cap
    let base_cap_center = vertices.len() as u32;
    vertices.push(PrimitiveVertex {
        pos: [0.0, -hy, 0.0],
        nrm: [0.0, -1.0, 0.0],
    });
    for i in 0..=seg {
        let a = (i as f32) * (2.0 * PI) / (seg as f32);
        let (sa, ca) = a.sin_cos();
        vertices.push(PrimitiveVertex {
            pos: [ca * r, -hy, sa * r],
            nrm: [0.0, -1.0, 0.0],
        });
    }

    let mut indices = Vec::with_capacity((seg as usize) * 6);

    // sides
    for i in 0..seg {
        let a = i;
        let b = i + 1;
        indices.extend_from_slice(&[a, b, apex_index]);
    }

    // base cap (wind opposite)
    for i in 0..seg {
        let a = base_cap_center + 1 + i;
        let b = base_cap_center + 1 + i + 1;
        indices.extend_from_slice(&[base_cap_center, b, a]);
    }

    PrimitiveMesh {
        vertices,
        indices,
        bounds_center: Vec3::ZERO,
        bounds_radius: Vec3::new(r, hy, 0.0).length(),
    }
}
