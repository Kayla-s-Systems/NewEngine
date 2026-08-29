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
    let rings = clamp_u32(params.rings, 2, 512);
    let r = 0.25f32;
    let cyl_hy = 0.25f32;
    let ring_count = (rings + 1) + 1 + rings;
    let ring_stride = seg + 1;
    let mut vertices = Vec::with_capacity((ring_count * ring_stride) as usize);
    let total_height = (cyl_hy + r) * 2.0;
    let mut push_ring = |y: f32, rr: f32, normal_center: Option<f32>| {
        let v = ((y + total_height * 0.5) / total_height).clamp(0.0, 1.0);
        for i in 0..=seg {
            let a = (i as f32) * (2.0 * PI) / (seg as f32);
            let (sa, ca) = a.sin_cos();
            let x = ca * rr;
            let z = sa * rr;
            let n = match normal_center {
                Some(cy) => {
                    let n0 = Vec3::new(x, y - cy, z);
                    n0 * (1.0 / n0.length().max(1.0e-20))
                }
                None => {
                    let n0 = Vec3::new(x, 0.0, z);
                    n0 * (1.0 / n0.length().max(1.0e-20))
                }
            };
            vertices.push(PrimitiveVertex {
                pos: [x, y, z],
                nrm: [n.x, n.y, n.z],
                uv: [(i as f32) / (seg as f32), v],
            });
        }
    };
    for j in 0..=rings {
        let t = (j as f32) / (rings as f32);
        let ang = t * (PI * 0.5);
        let (sa, ca) = ang.sin_cos();
        push_ring(cyl_hy + ca * r, sa * r, Some(cyl_hy));
    }
    push_ring(-cyl_hy, r, None);
    for j in 1..=rings {
        let t = (j as f32) / (rings as f32);
        let ang = (1.0 - t) * (PI * 0.5);
        let (sa, ca) = ang.sin_cos();
        push_ring(-cyl_hy - ca * r, sa * r, Some(-cyl_hy));
    }
    let mut indices = Vec::with_capacity(((ring_count - 1) * seg * 2) as usize * 3);
    for ring in 0..(ring_count - 1) {
        let a = ring * ring_stride;
        let b = (ring + 1) * ring_stride;
        for i in 0..seg {
            let i0 = a + i;
            let i1 = i0 + 1;
            let i2 = b + i;
            let i3 = i2 + 1;
            indices.extend_from_slice(&[i0, i2, i1, i1, i2, i3]);
        }
    }
    PrimitiveMesh {
        vertices,
        indices,
        bounds_center: Vec3::ZERO,
        bounds_radius: Vec3::new(r, cyl_hy + r, 0.0).length(),
    }
}
