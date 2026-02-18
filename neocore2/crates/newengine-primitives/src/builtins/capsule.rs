#![forbid(unsafe_op_in_unsafe_fn)]

use core::f32::consts::PI;

use newengine_math::Vec3;

use crate::registry::PrimitiveParams;
use crate::{PrimitiveMesh, PrimitiveVertex};

#[inline]
fn clamp_u32(v: u32, lo: u32, hi: u32) -> u32 {
    v.max(lo).min(hi)
}

/// Unit capsule centered at origin.
///
/// Geometry:
/// - radius = 0.25
/// - cylinder half-height = 0.25
/// - total height = 1.0
/// - axis = Y
///
/// Params:
/// - `segments` (default 32) : around Y
/// - `rings` (default 8) : hemisphere rings (smoothness)
#[inline]
pub fn build(params: &PrimitiveParams) -> PrimitiveMesh {
    let seg = clamp_u32(params.segments, 3, 4096);
    let rings = clamp_u32(params.rings, 2, 512);

    let r = 0.25f32;
    let cyl_hy = 0.25f32;

    // Ring count along Y:
    // - top hemisphere: 0..=rings (pole -> equator)
    // - bottom cylinder ring at -cyl_hy
    // - bottom hemisphere: 1..=rings (just below equator -> pole)
    let ring_count = (rings + 1) + 1 + rings;
    let ring_stride = seg + 1;

    let vtx_count = (ring_count * ring_stride) as usize;
    let tri_count = ((ring_count - 1) * seg * 2) as usize;
    let mut vertices = Vec::with_capacity(vtx_count);

    // Helper: push one ring.
    let mut push_ring = |y: f32, rr: f32, normal_center: Option<f32>| {
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
            });
        }
    };

    // Top hemisphere: pole -> equator at +cyl_hy.
    for j in 0..=rings {
        let t = (j as f32) / (rings as f32);
        let ang = t * (PI * 0.5); // 0..pi/2
        let (sa, ca) = ang.sin_cos();
        let y = cyl_hy + ca * r;
        let rr = sa * r;
        push_ring(y, rr, Some(cyl_hy));
    }

    // Cylinder bottom ring (equator) at -cyl_hy.
    push_ring(-cyl_hy, r, None);

    // Bottom hemisphere: just below equator -> pole.
    for j in 1..=rings {
        let t = (j as f32) / (rings as f32);
        // start at pi/2, end at 0
        let ang = (1.0 - t) * (PI * 0.5);
        let (sa, ca) = ang.sin_cos();
        let y = -cyl_hy - ca * r;
        let rr = sa * r;
        push_ring(y, rr, Some(-cyl_hy));
    }

    let mut indices = Vec::with_capacity(tri_count * 3);
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
