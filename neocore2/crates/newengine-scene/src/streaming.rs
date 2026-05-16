#![forbid(unsafe_op_in_unsafe_fn)]

use std::collections::BTreeSet;

use newengine_math::Vec3;

/// Integer world-streaming cell coordinate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SceneCellCoord {
    pub x: i32,
    pub z: i32,
}

impl SceneCellCoord {
    #[inline]
    pub fn from_world_pos(pos: Vec3, cell_size_x: f32, cell_size_z: f32) -> Self {
        let sx = cell_size_x.max(1.0);
        let sz = cell_size_z.max(1.0);
        Self { x: (pos.x / sx).round() as i32, z: (pos.z / sz).round() as i32 }
    }

    #[inline]
    pub fn center(self, cell_size_x: f32, cell_size_z: f32) -> Vec3 {
        Vec3::new(self.x as f32 * cell_size_x, 0.0, self.z as f32 * cell_size_z)
    }

    #[inline]
    pub const fn chebyshev_distance(self, other: Self) -> i32 {
        let dx = (self.x - other.x).abs();
        let dz = (self.z - other.z).abs();
        if dx > dz { dx } else { dz }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SceneStreamingBudget {
    pub resident_radius: i32,
    pub unload_radius: i32,
    pub max_commits_per_tick: usize,
}

impl Default for SceneStreamingBudget {
    #[inline]
    fn default() -> Self {
        Self { resident_radius: 1, unload_radius: 2, max_commits_per_tick: 1 }
    }
}

impl SceneStreamingBudget {
    #[inline]
    pub fn sanitized(self) -> Self {
        let resident_radius = self.resident_radius.clamp(0, 1);
        Self {
            resident_radius,
            unload_radius: self.unload_radius.clamp((resident_radius + 1).max(1), 2),
            max_commits_per_tick: self.max_commits_per_tick.clamp(1, 1),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct SceneResidencySet {
    cells: BTreeSet<SceneCellCoord>,
}

impl SceneResidencySet {
    #[inline]
    pub fn insert(&mut self, coord: SceneCellCoord) -> bool { self.cells.insert(coord) }

    #[inline]
    pub fn remove(&mut self, coord: &SceneCellCoord) -> bool { self.cells.remove(coord) }

    #[inline]
    pub fn contains(&self, coord: &SceneCellCoord) -> bool { self.cells.contains(coord) }

    #[inline]
    pub fn len(&self) -> usize { self.cells.len() }

    #[inline]
    pub fn desired_cells(center: SceneCellCoord, radius: i32) -> Vec<SceneCellCoord> {
        let radius = radius.clamp(0, 1);
        let mut desired = Vec::new();
        for z in (center.z - radius)..=(center.z + radius) {
            for x in (center.x - radius)..=(center.x + radius) {
                desired.push(SceneCellCoord { x, z });
            }
        }
        desired.sort_by_key(|coord| {
            let dx = coord.x - center.x;
            let dz = coord.z - center.z;
            (dx * dx + dz * dz, coord.x, coord.z)
        });
        desired
    }
}
