#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_math::Vec3;
use newengine_primitives::{PrimitiveMesh, PrimitiveVertex};

use crate::heightfield::HeightField;

impl HeightField {
    pub fn to_primitive_mesh(&self) -> PrimitiveMesh {
        let vx = self.vertex_count_x();
        let vz = self.vertex_count_z();
        let mut vertices = Vec::with_capacity(vx * vz);
        let mut indices = Vec::with_capacity((vx - 1) * (vz - 1) * 6);

        let uv_density_x = 0.18f32;
        let uv_density_z = 0.18f32;
        for z in 0..vz {
            for x in 0..vx {
                let p = self.local_position_at_grid(x, z);
                let n = self.normal_at_grid(x, z);
                vertices.push(PrimitiveVertex {
                    pos: [p.x, p.y, p.z],
                    nrm: [n.x, n.y, n.z],
                    uv: [p.x * uv_density_x, p.z * uv_density_z],
                });
            }
        }

        for z in 0..(vz - 1) {
            for x in 0..(vx - 1) {
                let i0 = self.index(x, z) as u32;
                let i1 = self.index(x + 1, z) as u32;
                let i2 = self.index(x, z + 1) as u32;
                let i3 = self.index(x + 1, z + 1) as u32;
                indices.extend_from_slice(&[i0, i2, i1, i1, i2, i3]);
            }
        }

        let bounds = self.local_bounds();
        PrimitiveMesh {
            vertices,
            indices,
            bounds_center: bounds.center(),
            bounds_radius: bounds.half_extents().length(),
        }
    }

    #[inline]
    fn normal_at_grid(&self, x: usize, z: usize) -> Vec3 {
        let xl = x.saturating_sub(1);
        let xr = (x + 1).min(self.vertex_count_x().saturating_sub(1));
        let zd = z.saturating_sub(1);
        let zu = (z + 1).min(self.vertex_count_z().saturating_sub(1));

        let left = self.local_position_at_grid(xl, z);
        let right = self.local_position_at_grid(xr, z);
        let down = self.local_position_at_grid(x, zd);
        let up = self.local_position_at_grid(x, zu);

        let dx = right - left;
        let dz = up - down;
        dz.cross(dx).normalize_or_zero()
    }
}
