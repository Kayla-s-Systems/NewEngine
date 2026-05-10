#![forbid(unsafe_op_in_unsafe_fn)]

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use newengine_math::Vec3;

use crate::heightfield::HeightField;

#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct TerrainCollisionTileSettings {
    /// Quads per generated AABB collider tile.
    pub tile_cells: u32,
    /// Extra downward volume so player collision cannot fall through steep tile seams.
    pub floor_depth: f32,
    /// Small expansion along X/Z to avoid cracks between coarse collision proxies.
    pub horizontal_skin: f32,
}

impl Default for TerrainCollisionTileSettings {
    #[inline]
    fn default() -> Self {
        Self {
            tile_cells: 4,
            floor_depth: 2.0,
            horizontal_skin: 0.05,
        }
    }
}

impl TerrainCollisionTileSettings {
    #[inline]
    pub fn sanitized(self) -> Self {
        Self {
            tile_cells: self.tile_cells.clamp(1, 64),
            floor_depth: finite_or(self.floor_depth, 2.0).abs().max(0.05),
            horizontal_skin: finite_or(self.horizontal_skin, 0.05).abs().min(1.0),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TerrainCollisionTile {
    pub center: Vec3,
    pub half_extents: Vec3,
}

impl HeightField {
    pub fn collision_tiles(&self, settings: TerrainCollisionTileSettings) -> Vec<TerrainCollisionTile> {
        let settings = settings.sanitized();
        let tile = settings.tile_cells as usize;
        let sx = self.settings().size_x / self.settings().cells_x as f32;
        let sz = self.settings().size_z / self.settings().cells_z as f32;
        let mut out = Vec::new();

        let mut z0 = 0usize;
        while z0 < self.settings().cells_z as usize {
            let z1 = (z0 + tile).min(self.settings().cells_z as usize);
            let mut x0 = 0usize;
            while x0 < self.settings().cells_x as usize {
                let x1 = (x0 + tile).min(self.settings().cells_x as usize);
                let mut min_h = f32::INFINITY;
                let mut max_h = f32::NEG_INFINITY;

                for z in z0..=z1 {
                    for x in x0..=x1 {
                        let h = self.height_at_grid(x, z);
                        min_h = min_h.min(h);
                        max_h = max_h.max(h);
                    }
                }

                let min_x = -self.settings().size_x * 0.5 + x0 as f32 * sx;
                let max_x = -self.settings().size_x * 0.5 + x1 as f32 * sx;
                let min_z = -self.settings().size_z * 0.5 + z0 as f32 * sz;
                let max_z = -self.settings().size_z * 0.5 + z1 as f32 * sz;
                let bottom = min_h - settings.floor_depth;
                let top = max_h + 0.05;

                out.push(TerrainCollisionTile {
                    center: Vec3::new(
                        (min_x + max_x) * 0.5,
                        (bottom + top) * 0.5,
                        (min_z + max_z) * 0.5,
                    ),
                    half_extents: Vec3::new(
                        ((max_x - min_x) * 0.5 + settings.horizontal_skin).max(0.05),
                        ((top - bottom) * 0.5).max(0.05),
                        ((max_z - min_z) * 0.5 + settings.horizontal_skin).max(0.05),
                    ),
                });

                x0 = x1;
            }
            z0 = z1;
        }

        out
    }
}

#[inline]
fn finite_or(v: f32, fallback: f32) -> f32 {
    if v.is_finite() { v } else { fallback }
}
