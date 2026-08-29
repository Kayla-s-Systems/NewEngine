#![forbid(unsafe_op_in_unsafe_fn)]

use crate::registry::PrimitiveParams;
use crate::{PrimitiveMesh, PrimitiveVertex};
use core::f32::consts::PI;
use newengine_math::Vec3;

#[inline]
fn clamp_u32(v: u32, lo: u32, hi: u32) -> u32 {
    v.max(lo).min(hi)
}

#[inline]
pub fn build(params: &PrimitiveParams) -> PrimitiveMesh {
    let seg = clamp_u32(params.segments, 3, 4096);
    let r = 0.5f32;
    let hy = 0.5f32;
    let mut vertices = Vec::with_capacity(((seg + 1) + 1 + (seg + 1) + 1) as usize);
    let slope = r / (2.0 * hy);
    for i in 0..=seg {
        let a = (i as f32) * (2.0 * PI) / (seg as f32);
        let (sa, ca) = a.sin_cos();
        let n0 = Vec3::new(ca, slope, sa);
        let n = n0 * (1.0 / n0.length().max(1.0e-20));
        vertices.push(PrimitiveVertex {
            pos: [ca * r, -hy, sa * r],
            nrm: [n.x, n.y, n.z],
            uv: [(i as f32) / (seg as f32), 0.0],
        });
    }
    let apex_index = vertices.len() as u32;
    vertices.push(PrimitiveVertex {
        pos: [0.0, hy, 0.0],
        nrm: [0.0, 1.0, 0.0],
        uv: [0.5, 1.0],
    });
    let base_cap_center = vertices.len() as u32;
    vertices.push(PrimitiveVertex {
        pos: [0.0, -hy, 0.0],
        nrm: [0.0, -1.0, 0.0],
        uv: [0.5, 0.5],
    });
    for i in 0..=seg {
        let a = (i as f32) * (2.0 * PI) / (seg as f32);
        let (sa, ca) = a.sin_cos();
        vertices.push(PrimitiveVertex {
            pos: [ca * r, -hy, sa * r],
            nrm: [0.0, -1.0, 0.0],
            uv: [ca * 0.5 + 0.5, sa * 0.5 + 0.5],
        });
    }
    let mut indices = Vec::with_capacity((seg as usize) * 6);
    for i in 0..seg {
        indices.extend_from_slice(&[i, i + 1, apex_index]);
    }
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
