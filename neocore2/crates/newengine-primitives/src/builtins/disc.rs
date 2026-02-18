#![forbid(unsafe_op_in_unsafe_fn)]

use core::f32::consts::PI;

use newengine_math::Vec3;

use crate::registry::PrimitiveParams;
use crate::{PrimitiveMesh, PrimitiveVertex};

#[inline]
fn clamp_u32(v: u32, lo: u32, hi: u32) -> u32 {
    v.max(lo).min(hi)
}

/// Unit disc on XZ plane, centered at origin, normal +Y.
///
/// - radius = 0.5
/// Params:
/// - `segments` (default 48)
#[inline]
pub fn build(params: &PrimitiveParams) -> PrimitiveMesh {
    let seg = clamp_u32(params.segments, 3, 4096);
    let r = 0.5f32;

    // center + ring
    let mut vertices = Vec::with_capacity((seg + 1) as usize);
    vertices.push(PrimitiveVertex {
        pos: [0.0, 0.0, 0.0],
        nrm: [0.0, 1.0, 0.0],
    });

    for i in 0..seg {
        let a = (i as f32) * (2.0 * PI) / (seg as f32);
        let (sa, ca) = a.sin_cos();
        vertices.push(PrimitiveVertex {
            pos: [ca * r, 0.0, sa * r],
            nrm: [0.0, 1.0, 0.0],
        });
    }

    let mut indices = Vec::with_capacity((seg as usize) * 3);
    for i in 0..seg {
        let a = 1 + i;
        let b = 1 + ((i + 1) % seg);
        indices.extend_from_slice(&[0, b, a]);
    }

    PrimitiveMesh {
        vertices,
        indices,
        bounds_center: Vec3::ZERO,
        bounds_radius: r,
    }
}
