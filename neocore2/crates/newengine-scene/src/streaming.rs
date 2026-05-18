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
        Self { x: (pos.x / sx).floor() as i32, z: (pos.z / sz).floor() as i32 }
    }

    #[inline]
    pub fn center(self, cell_size_x: f32, cell_size_z: f32) -> Vec3 {
        Vec3::new(
            (self.x as f32 + 0.5) * cell_size_x,
            0.0,
            (self.z as f32 + 0.5) * cell_size_z,
        )
    }

    #[inline]
    pub const fn chebyshev_distance(self, other: Self) -> i32 {
        let dx = (self.x - other.x).abs();
        let dz = (self.z - other.z).abs();
        if dx > dz { dx } else { dz }
    }

    #[inline]
    pub const fn distance_key(self, other: Self) -> (i32, i32, i32, i32) {
        let dx = self.x - other.x;
        let dz = self.z - other.z;
        let ax = dx.abs();
        let az = dz.abs();
        let chebyshev = if ax > az { ax } else { az };
        (dx * dx + dz * dz, chebyshev, self.x, self.z)
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
        Self { resident_radius: 2, unload_radius: 4, max_commits_per_tick: 4 }
    }
}

impl SceneStreamingBudget {
    pub const MAX_RESIDENT_RADIUS: i32 = 8;
    pub const MAX_UNLOAD_RADIUS: i32 = 12;
    pub const MAX_COMMITS_PER_TICK: usize = 16;

    #[inline]
    pub fn sanitized(self) -> Self {
        let resident_radius = self.resident_radius.clamp(0, Self::MAX_RESIDENT_RADIUS);
        Self {
            resident_radius,
            unload_radius: self
                .unload_radius
                .clamp((resident_radius + 1).max(1), Self::MAX_UNLOAD_RADIUS.max(resident_radius + 1)),
            max_commits_per_tick: self.max_commits_per_tick.clamp(1, Self::MAX_COMMITS_PER_TICK),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SceneStreamingRequestKind {
    Load,
    Unload,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SceneStreamingRequest {
    pub kind: SceneStreamingRequestKind,
    pub coord: SceneCellCoord,
    pub priority_key: (i32, i32, i32, i32),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SceneStreamingPlan {
    pub center: SceneCellCoord,
    pub budget: SceneStreamingBudget,
    pub desired: Vec<SceneCellCoord>,
    pub loads: Vec<SceneStreamingRequest>,
    pub unloads: Vec<SceneStreamingRequest>,
}

impl SceneStreamingPlan {
    pub fn build(
        center: SceneCellCoord,
        budget: SceneStreamingBudget,
        loaded: impl IntoIterator<Item = SceneCellCoord>,
        pending: impl IntoIterator<Item = SceneCellCoord>,
    ) -> Self {
        let budget = budget.sanitized();
        let desired = SceneResidencySet::desired_cells(center, budget.resident_radius);
        let loaded_set = loaded.into_iter().collect::<BTreeSet<_>>();
        let pending_set = pending.into_iter().collect::<BTreeSet<_>>();

        let mut loads = desired
            .iter()
            .copied()
            .filter(|coord| !loaded_set.contains(coord) && !pending_set.contains(coord))
            .map(|coord| SceneStreamingRequest {
                kind: SceneStreamingRequestKind::Load,
                coord,
                priority_key: coord.distance_key(center),
            })
            .collect::<Vec<_>>();
        loads.sort_by_key(|request| request.priority_key);

        let mut unloads = loaded_set
            .iter()
            .copied()
            .filter(|coord| coord.chebyshev_distance(center) > budget.unload_radius)
            .map(|coord| SceneStreamingRequest {
                kind: SceneStreamingRequestKind::Unload,
                coord,
                priority_key: coord.distance_key(center),
            })
            .collect::<Vec<_>>();
        unloads.sort_by(|a, b| b.priority_key.cmp(&a.priority_key));

        Self { center, budget, desired, loads, unloads }
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
    pub fn is_empty(&self) -> bool { self.cells.is_empty() }

    #[inline]
    pub fn desired_cells(center: SceneCellCoord, radius: i32) -> Vec<SceneCellCoord> {
        let radius = radius.clamp(0, SceneStreamingBudget::MAX_RESIDENT_RADIUS);
        let side = (radius as usize).saturating_mul(2).saturating_add(1);
        let mut desired = Vec::with_capacity(side.saturating_mul(side));
        for z in (center.z - radius)..=(center.z + radius) {
            for x in (center.x - radius)..=(center.x + radius) {
                desired.push(SceneCellCoord { x, z });
            }
        }
        desired.sort_by_key(|coord| coord.distance_key(center));
        desired
    }
}
