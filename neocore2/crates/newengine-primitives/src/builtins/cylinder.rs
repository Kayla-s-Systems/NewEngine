#![forbid(unsafe_op_in_unsafe_fn)]

use core::f32::consts::PI;

use newengine_math::Vec3;

use crate::registry::PrimitiveParams;
use crate::{PrimitiveMesh, PrimitiveVertex};

#[inline]
fn clamp_u32(v: u32, lo: u32, hi: u32) -> u32 { v.max(lo).min(hi) }

#[inline]
pub fn build(params: &PrimitiveParams) -> PrimitiveMesh {
    let seg = clamp_u32(params.segments, 3, 4096);
    let r = 0.5f32;
    let hy = 0.5f32;
    let mut vertices = Vec::with_capacity((((seg + 1) * 2) + (((seg + 1) + 1) * 2)) as usize);
    for i in 0..=seg {
        let a = (i as f32) * (2.0 * PI) / (seg as f32);
        let (sa, ca) = a.sin_cos();
        let u = (i as f32) / (seg as f32);
        let px = ca * r;
        let pz = sa * r;
        vertices.push(PrimitiveVertex { pos: [px, -hy, pz], nrm: [ca, 0.0, sa], uv: [u, 0.0] });
        vertices.push(PrimitiveVertex { pos: [px,  hy, pz], nrm: [ca, 0.0, sa], uv: [u, 1.0] });
    }
    let side_base = 0u32;
    let cap_top_base = vertices.len() as u32;
    vertices.push(PrimitiveVertex { pos: [0.0, hy, 0.0], nrm: [0.0, 1.0, 0.0], uv: [0.5, 0.5] });
    for i in 0..=seg {
        let a = (i as f32) * (2.0 * PI) / (seg as f32);
        let (sa, ca) = a.sin_cos();
        vertices.push(PrimitiveVertex { pos: [ca * r, hy, sa * r], nrm: [0.0, 1.0, 0.0], uv: [ca * 0.5 + 0.5, sa * 0.5 + 0.5] });
    }
    let cap_bottom_base = vertices.len() as u32;
    vertices.push(PrimitiveVertex { pos: [0.0, -hy, 0.0], nrm: [0.0, -1.0, 0.0], uv: [0.5, 0.5] });
    for i in 0..=seg {
        let a = (i as f32) * (2.0 * PI) / (seg as f32);
        let (sa, ca) = a.sin_cos();
        vertices.push(PrimitiveVertex { pos: [ca * r, -hy, sa * r], nrm: [0.0, -1.0, 0.0], uv: [ca * 0.5 + 0.5, sa * 0.5 + 0.5] });
    }
    let mut indices = Vec::with_capacity(((seg as usize) * 12) + ((seg as usize) * 6));
    for i in 0..seg {
        let b0 = side_base + i * 2;
        let b1 = b0 + 1;
        let c0 = b0 + 2;
        let c1 = b1 + 2;
        indices.extend_from_slice(&[b0, b1, c0, c0, b1, c1]);
    }
    for i in 0..seg {
        let a = cap_top_base + 1 + i;
        let b = cap_top_base + 1 + i + 1;
        indices.extend_from_slice(&[cap_top_base, a, b]);
    }
    for i in 0..seg {
        let a = cap_bottom_base + 1 + i;
        let b = cap_bottom_base + 1 + i + 1;
        indices.extend_from_slice(&[cap_bottom_base, b, a]);
    }
    PrimitiveMesh { vertices, indices, bounds_center: Vec3::ZERO, bounds_radius: Vec3::new(r, hy, 0.0).length() }
}
