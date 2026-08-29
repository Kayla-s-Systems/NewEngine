#![forbid(unsafe_op_in_unsafe_fn)]

use core::f32::consts::PI;

use newengine_math::Vec3;

use crate::registry::PrimitiveParams;
use crate::{PrimitiveMesh, PrimitiveVertex};

#[inline]
fn clamp_u32(v: u32, lo: u32, hi: u32) -> u32 {
    v.max(lo).min(hi)
}

#[inline]
pub fn build(params: &PrimitiveParams) -> PrimitiveMesh {
    let slices = clamp_u32(params.slices, 3, 512);
    let stacks = clamp_u32(params.stacks, 2, 512);
    let radius = 0.5f32;
    let vert_w = slices + 1;
    let vert_h = stacks + 1;
    let mut vertices = Vec::with_capacity((vert_w * vert_h) as usize);
    for y in 0..=stacks {
        let v = (y as f32) / (stacks as f32);
        let phi = v * PI;
        let (sp, cp) = phi.sin_cos();
        for x in 0..=slices {
            let u = (x as f32) / (slices as f32);
            let theta = u * (2.0 * PI);
            let (st, ct) = theta.sin_cos();
            let n = Vec3::new(ct * sp, cp, st * sp);
            let p = n * radius;
            vertices.push(PrimitiveVertex {
                pos: [p.x, p.y, p.z],
                nrm: [n.x, n.y, n.z],
                uv: [u, 1.0 - v],
            });
        }
    }
    let mut indices = Vec::with_capacity((slices * stacks * 2) as usize * 3);
    for y in 0..stacks {
        for x in 0..slices {
            let i0 = y * vert_w + x;
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
        bounds_radius: radius,
    }
}
