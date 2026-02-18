#![forbid(unsafe_op_in_unsafe_fn)]

use core::f32::consts::PI;

use newengine_math::Vec3;

use crate::registry::PrimitiveParams;
use crate::{PrimitiveMesh, PrimitiveVertex};

#[inline]
fn clamp_u32(v: u32, lo: u32, hi: u32) -> u32 {
    v.max(lo).min(hi)
}

/// Unit cylinder centered at origin.
///
/// - radius = 0.5
/// - height = 1.0
/// - axis = Y
///
/// Params:
/// - `segments` (default 32)
#[inline]
pub fn build(params: &PrimitiveParams) -> PrimitiveMesh {
    let seg = clamp_u32(params.segments, 3, 4096);
    let r = 0.5f32;
    let hy = 0.5f32;

    // side: (seg+1)*2 vertices
    let side_vtx = ((seg + 1) * 2) as usize;
    // caps: (seg+1) ring vertices + center, twice
    let cap_vtx = ((seg + 1) + 1) as usize;
    let mut vertices = Vec::with_capacity(side_vtx + cap_vtx * 2);

    // side vertices (duplicate seam)
    for i in 0..=seg {
        let a = (i as f32) * (2.0 * PI) / (seg as f32);
        let (sa, ca) = a.sin_cos();
        let nx = ca;
        let nz = sa;
        let px = ca * r;
        let pz = sa * r;

        vertices.push(PrimitiveVertex {
            pos: [px, -hy, pz],
            nrm: [nx, 0.0, nz],
        });
        vertices.push(PrimitiveVertex {
            pos: [px, hy, pz],
            nrm: [nx, 0.0, nz],
        });
    }

    let side_base = 0u32;
    let cap_top_base = vertices.len() as u32;

    // top cap
    vertices.push(PrimitiveVertex {
        pos: [0.0, hy, 0.0],
        nrm: [0.0, 1.0, 0.0],
    });
    for i in 0..=seg {
        let a = (i as f32) * (2.0 * PI) / (seg as f32);
        let (sa, ca) = a.sin_cos();
        vertices.push(PrimitiveVertex {
            pos: [ca * r, hy, sa * r],
            nrm: [0.0, 1.0, 0.0],
        });
    }

    let cap_bottom_base = vertices.len() as u32;

    // bottom cap
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

    // indices
    let mut indices = Vec::with_capacity(((seg as usize) * 12) + ((seg as usize) * 6));

    // sides
    for i in 0..seg {
        let b0 = side_base + i * 2;
        let b1 = b0 + 1;
        let b2 = b0 + 2;
        let b3 = b0 + 3;
        indices.extend_from_slice(&[b0, b2, b1, b1, b2, b3]);
    }

    // top cap: center is at cap_top_base
    for i in 0..seg {
        let c = cap_top_base;
        let a = cap_top_base + 1 + i;
        let b = cap_top_base + 1 + i + 1;
        indices.extend_from_slice(&[c, a, b]);
    }

    // bottom cap: wind opposite
    for i in 0..seg {
        let c = cap_bottom_base;
        let a = cap_bottom_base + 1 + i;
        let b = cap_bottom_base + 1 + i + 1;
        indices.extend_from_slice(&[c, b, a]);
    }

    PrimitiveMesh {
        vertices,
        indices,
        bounds_center: Vec3::ZERO,
        bounds_radius: Vec3::new(r, hy, 0.0).length(),
    }
}
