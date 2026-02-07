#![forbid(unsafe_op_in_unsafe_fn)]

use glam::{Mat4, Vec3, Vec4};

/// View frustum extracted from a view-projection matrix.
///
/// Planes are normalized: ax + by + cz + d >= 0 is inside.
#[derive(Clone, Copy, Debug)]
pub struct Frustum {
    pub planes: [Vec4; 6],
}

impl Frustum {
    /// Extract planes from a column-major clip matrix (proj * view).
    #[inline]
    pub fn from_view_proj(m: Mat4) -> Self {
        // glam Mat4 stores columns; convert to rows by indexing.
        let r0 = Vec4::new(m.x_axis.x, m.y_axis.x, m.z_axis.x, m.w_axis.x);
        let r1 = Vec4::new(m.x_axis.y, m.y_axis.y, m.z_axis.y, m.w_axis.y);
        let r2 = Vec4::new(m.x_axis.z, m.y_axis.z, m.z_axis.z, m.w_axis.z);
        let r3 = Vec4::new(m.x_axis.w, m.y_axis.w, m.z_axis.w, m.w_axis.w);

        let mut p = [
            r3 + r0, // left
            r3 - r0, // right
            r3 + r1, // bottom
            r3 - r1, // top
            r3 + r2, // near
            r3 - r2, // far
        ];

        for pl in &mut p {
            let n = Vec3::new(pl.x, pl.y, pl.z);
            let inv_len = 1.0 / n.length().max(1e-6);
            *pl *= inv_len;
        }

        Self { planes: p }
    }

    #[inline]
    pub fn contains_sphere(&self, center: Vec3, radius: f32) -> bool {
        let c = Vec4::new(center.x, center.y, center.z, 1.0);
        for pl in &self.planes {
            if pl.dot(c) < -radius {
                return false;
            }
        }
        true
    }

    #[inline]
    pub fn contains_aabb(&self, min: Vec3, max: Vec3) -> bool {
        for pl in &self.planes {
            let n = Vec3::new(pl.x, pl.y, pl.z);
            let p = Vec3::new(
                if n.x >= 0.0 { max.x } else { min.x },
                if n.y >= 0.0 { max.y } else { min.y },
                if n.z >= 0.0 { max.z } else { min.z },
            );
            if n.dot(p) + pl.w < 0.0 {
                return false;
            }
        }
        true
    }
}