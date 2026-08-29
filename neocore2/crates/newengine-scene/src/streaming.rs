#![forbid(unsafe_op_in_unsafe_fn)]

use std::{
    collections::{BTreeSet, HashSet},
    sync::OnceLock,
};

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
        Self {
            x: (pos.x / sx).floor() as i32,
            z: (pos.z / sz).floor() as i32,
        }
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
        if dx > dz {
            dx
        } else {
            dz
        }
    }

    #[inline]
    pub const fn manhattan_distance(self, other: Self) -> i32 {
        (self.x - other.x).abs() + (self.z - other.z).abs()
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

/// Simple radius/budget contract kept as the compact radius/budget contract for scene profiles.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SceneStreamingBudget {
    pub resident_radius: i32,
    pub unload_radius: i32,
    pub max_commits_per_tick: usize,
}

impl Default for SceneStreamingBudget {
    #[inline]
    fn default() -> Self {
        Self {
            resident_radius: 2,
            unload_radius: 4,
            max_commits_per_tick: 4,
        }
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
            unload_radius: self.unload_radius.clamp(
                (resident_radius + 1).max(1),
                Self::MAX_UNLOAD_RADIUS.max(resident_radius + 1),
            ),
            max_commits_per_tick: self
                .max_commits_per_tick
                .clamp(1, Self::MAX_COMMITS_PER_TICK),
        }
    }
}

/// Scene residency layer. Render residency answers "what can be drawn now".
/// Simulation residency answers "what must keep ticking even if it is invisible".
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SceneResidencyLayer {
    Render,
    Simulation,
}

/// Focus point used by streaming planners.
///
/// AAA-style streaming is not pure distance-to-player. It scores cells from a
/// focus position, optional forward/read-ahead direction and an explicit layer.
/// Render and simulation can therefore diverge: a city block can keep coarse AI
/// simulation without holding GPU meshes/textures resident.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SceneStreamingObserver {
    pub position: Vec3,
    pub forward: Vec3,
    pub velocity: Vec3,
    pub read_ahead_seconds: f32,
}

impl SceneStreamingObserver {
    #[inline]
    pub fn at(position: Vec3) -> Self {
        Self {
            position,
            forward: Vec3::new(0.0, 0.0, 1.0),
            velocity: Vec3::ZERO,
            read_ahead_seconds: 0.0,
        }
    }

    #[inline]
    pub fn with_motion(mut self, forward: Vec3, velocity: Vec3, read_ahead_seconds: f32) -> Self {
        self.forward = forward;
        self.velocity = velocity;
        self.read_ahead_seconds = read_ahead_seconds.max(0.0);
        self
    }

    #[inline]
    pub fn focus_position(self) -> Vec3 {
        self.position + self.velocity * self.read_ahead_seconds
    }

    #[inline]
    pub fn cell(self, cell_size_x: f32, cell_size_z: f32) -> SceneCellCoord {
        SceneCellCoord::from_world_pos(self.focus_position(), cell_size_x, cell_size_z)
    }
}

/// Dual-layer scene streaming policy.
///
/// Render radius should usually be small and visibility/frustum driven.
/// Simulation radius may be wider, but should use cheaper simulation LODs and
/// must not imply render/GPU residency.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SceneStreamingProfile {
    pub render: SceneStreamingBudget,
    pub simulation: SceneStreamingBudget,
}

impl Default for SceneStreamingProfile {
    #[inline]
    fn default() -> Self {
        Self {
            render: SceneStreamingBudget::default(),
            simulation: SceneStreamingBudget {
                resident_radius: 4,
                unload_radius: 6,
                max_commits_per_tick: 2,
            },
        }
    }
}

impl SceneStreamingProfile {
    #[inline]
    pub fn sanitized(self) -> Self {
        let render = self.render.sanitized();
        let mut simulation = self.simulation.sanitized();
        if simulation.resident_radius < render.resident_radius {
            simulation.resident_radius = render.resident_radius;
        }
        if simulation.unload_radius < render.unload_radius {
            simulation.unload_radius = render.unload_radius;
        }
        Self { render, simulation }
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
        Self::build_from_desired(center, budget, desired, loaded, pending)
    }

    /// Build residency from a primary focus plus one or more bounded secondary focuses.
    ///
    /// Mature open-world streamers issue spatial searches from more than one point: the
    /// current player/camera position remains authoritative while velocity/cutscene/vehicle
    /// look-ahead positions can prewarm a smaller region. Keeping the primary center for
    /// unload scoring prevents read-ahead from evicting the ground directly under the player.
    pub fn build_multi_focus(
        center: SceneCellCoord,
        budget: SceneStreamingBudget,
        secondary_focuses: impl IntoIterator<Item = (SceneCellCoord, i32)>,
        loaded: impl IntoIterator<Item = SceneCellCoord>,
        pending: impl IntoIterator<Item = SceneCellCoord>,
    ) -> Self {
        let budget = budget.sanitized();
        let desired = SceneResidencySet::desired_cells_for_focuses(
            center,
            budget.resident_radius,
            secondary_focuses,
        );
        Self::build_from_desired(center, budget, desired, loaded, pending)
    }

    pub fn build_from_desired(
        center: SceneCellCoord,
        budget: SceneStreamingBudget,
        mut desired: Vec<SceneCellCoord>,
        loaded: impl IntoIterator<Item = SceneCellCoord>,
        pending: impl IntoIterator<Item = SceneCellCoord>,
    ) -> Self {
        let budget = budget.sanitized();
        desired.sort_by_key(|coord| coord.distance_key(center));
        desired.dedup();
        // These sets are frame-local membership accelerators. Request ordering is sorted below,
        // so tree ordering buys us nothing here while imposing O(log N) lookup/build costs.
        let loaded_set = loaded.into_iter().collect::<HashSet<_>>();
        let pending_set = pending.into_iter().collect::<HashSet<_>>();
        let desired_set = desired.iter().copied().collect::<HashSet<_>>();

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
            .filter(|coord| {
                !desired_set.contains(coord)
                    && coord.chebyshev_distance(center) > budget.unload_radius
            })
            .map(|coord| SceneStreamingRequest {
                kind: SceneStreamingRequestKind::Unload,
                coord,
                priority_key: coord.distance_key(center),
            })
            .collect::<Vec<_>>();
        unloads.sort_by(|a, b| b.priority_key.cmp(&a.priority_key));

        Self {
            center,
            budget,
            desired,
            loads,
            unloads,
        }
    }
}

/// A two-layer plan produced from the same observer.
///
/// The render plan is intentionally allowed to be a subset of simulation. This
/// is the key distinction needed by large worlds: invisible cells can keep cheap
/// gameplay state alive without also owning render meshes, textures or draw-list
/// entries.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SceneLayeredStreamingPlan {
    pub center: SceneCellCoord,
    pub render: SceneStreamingPlan,
    pub simulation: SceneStreamingPlan,
}

impl SceneLayeredStreamingPlan {
    pub fn build(
        center: SceneCellCoord,
        profile: SceneStreamingProfile,
        render_loaded: impl IntoIterator<Item = SceneCellCoord>,
        render_pending: impl IntoIterator<Item = SceneCellCoord>,
        simulation_loaded: impl IntoIterator<Item = SceneCellCoord>,
        simulation_pending: impl IntoIterator<Item = SceneCellCoord>,
    ) -> Self {
        let profile = profile.sanitized();
        let render =
            SceneStreamingPlan::build(center, profile.render, render_loaded, render_pending);
        let simulation = SceneStreamingPlan::build(
            center,
            profile.simulation,
            simulation_loaded,
            simulation_pending,
        );
        Self {
            center,
            render,
            simulation,
        }
    }

    /// Build the render/simulation layers from caller-authored desired sets.
    ///
    /// This is the bridge used by spatial read-ahead: the planner still owns deterministic
    /// request/unload ordering and hysteresis, while the world streamer can union primary
    /// and predictive search volumes before admission.
    pub fn build_from_desired(
        center: SceneCellCoord,
        profile: SceneStreamingProfile,
        render_desired: Vec<SceneCellCoord>,
        simulation_desired: Vec<SceneCellCoord>,
        render_loaded: impl IntoIterator<Item = SceneCellCoord>,
        render_pending: impl IntoIterator<Item = SceneCellCoord>,
        simulation_loaded: impl IntoIterator<Item = SceneCellCoord>,
        simulation_pending: impl IntoIterator<Item = SceneCellCoord>,
    ) -> Self {
        let profile = profile.sanitized();
        let render = SceneStreamingPlan::build_from_desired(
            center,
            profile.render,
            render_desired,
            render_loaded,
            render_pending,
        );
        let simulation = SceneStreamingPlan::build_from_desired(
            center,
            profile.simulation,
            simulation_desired,
            simulation_loaded,
            simulation_pending,
        );
        Self {
            center,
            render,
            simulation,
        }
    }
}

/// Coarse active-scene bucket inspired by mature open-world streamers.
///
/// The bucket is a policy label, not a renderer command. Scene streaming uses it
/// to separate "must be fully resident now" from "can be kept as simulation or
/// delayed render work". Higher numeric priority means more urgent residency.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SceneStreamingBucket {
    ActiveSimulation,
    VisibleNear,
    VisibleFar,
    PredictedNear,
    SimulationOnly,
    InvisibleFar,
    Sleeping,
}

impl SceneStreamingBucket {
    #[inline]
    pub const fn priority(self) -> i32 {
        match self {
            Self::ActiveSimulation => 700,
            Self::VisibleNear => 650,
            Self::PredictedNear => 580,
            Self::VisibleFar => 520,
            Self::SimulationOnly => 360,
            Self::InvisibleFar => 160,
            Self::Sleeping => 0,
        }
    }

    #[inline]
    pub const fn wants_render_residency(self) -> bool {
        matches!(
            self,
            Self::VisibleNear | Self::VisibleFar | Self::PredictedNear
        )
    }

    #[inline]
    pub const fn wants_simulation_residency(self) -> bool {
        !matches!(self, Self::InvisibleFar | Self::Sleeping)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SceneBucketedCell {
    pub coord: SceneCellCoord,
    pub bucket: SceneStreamingBucket,
    pub score: i32,
}

impl SceneBucketedCell {
    #[inline]
    pub fn new(
        coord: SceneCellCoord,
        bucket: SceneStreamingBucket,
        center: SceneCellCoord,
    ) -> Self {
        let distance_penalty = coord.manhattan_distance(center).saturating_mul(12);
        Self {
            coord,
            bucket,
            score: bucket.priority().saturating_sub(distance_penalty),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SceneBucketedCellPlan {
    pub cells: Vec<SceneBucketedCell>,
}

impl SceneBucketedCellPlan {
    pub fn from_desired_sets(
        center: SceneCellCoord,
        render_desired: impl IntoIterator<Item = SceneCellCoord>,
        simulation_desired: impl IntoIterator<Item = SceneCellCoord>,
    ) -> Self {
        Self::from_desired_sets_with_prediction(
            center,
            render_desired,
            simulation_desired,
            std::iter::empty(),
        )
    }

    /// Classify desired cells while preserving a distinct predictive bucket.
    ///
    /// Predictive cells are intentionally admitted after the immediate near ring but before
    /// ordinary far render residency. This mirrors request-list streamers that prefetch along
    /// motion/camera paths without allowing speculative work to starve the current scene.
    pub fn from_desired_sets_with_prediction(
        center: SceneCellCoord,
        render_desired: impl IntoIterator<Item = SceneCellCoord>,
        simulation_desired: impl IntoIterator<Item = SceneCellCoord>,
        predicted_desired: impl IntoIterator<Item = SceneCellCoord>,
    ) -> Self {
        let render_set = render_desired.into_iter().collect::<HashSet<_>>();
        let simulation_set = simulation_desired.into_iter().collect::<HashSet<_>>();
        let predicted_set = predicted_desired.into_iter().collect::<HashSet<_>>();
        let mut all = render_set
            .union(&simulation_set)
            .copied()
            .collect::<Vec<_>>();
        all.sort_by_key(|coord| coord.distance_key(center));
        all.dedup();

        let mut cells = all
            .into_iter()
            .map(|coord| {
                let dist = coord.chebyshev_distance(center);
                let bucket = if render_set.contains(&coord) {
                    if dist <= 1 {
                        SceneStreamingBucket::VisibleNear
                    } else if predicted_set.contains(&coord) {
                        SceneStreamingBucket::PredictedNear
                    } else {
                        SceneStreamingBucket::VisibleFar
                    }
                } else if simulation_set.contains(&coord) {
                    SceneStreamingBucket::SimulationOnly
                } else {
                    SceneStreamingBucket::Sleeping
                };
                SceneBucketedCell::new(coord, bucket, center)
            })
            .collect::<Vec<_>>();
        cells.sort_by(|a, b| {
            b.score.cmp(&a.score).then_with(|| {
                a.coord
                    .distance_key(center)
                    .cmp(&b.coord.distance_key(center))
            })
        });
        Self { cells }
    }
}

#[derive(Clone, Debug, Default)]
pub struct SceneResidencySet {
    cells: BTreeSet<SceneCellCoord>,
}

impl SceneResidencySet {
    #[inline]
    pub fn insert(&mut self, coord: SceneCellCoord) -> bool {
        self.cells.insert(coord)
    }

    #[inline]
    pub fn remove(&mut self, coord: &SceneCellCoord) -> bool {
        self.cells.remove(coord)
    }

    #[inline]
    pub fn contains(&self, coord: &SceneCellCoord) -> bool {
        self.cells.contains(coord)
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.cells.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }

    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = SceneCellCoord> + '_ {
        self.cells.iter().copied()
    }

    #[inline]
    pub fn desired_cells(center: SceneCellCoord, radius: i32) -> Vec<SceneCellCoord> {
        let radius = radius.clamp(0, SceneStreamingBudget::MAX_RESIDENT_RADIUS) as usize;
        ordered_residency_offsets()[radius]
            .iter()
            .map(|offset| SceneCellCoord {
                x: center.x.saturating_add(offset.x),
                z: center.z.saturating_add(offset.z),
            })
            .collect()
    }

    pub fn desired_cells_for_focuses(
        center: SceneCellCoord,
        radius: i32,
        secondary_focuses: impl IntoIterator<Item = (SceneCellCoord, i32)>,
    ) -> Vec<SceneCellCoord> {
        let mut desired = Self::desired_cells(center, radius)
            .into_iter()
            .collect::<HashSet<_>>();
        for (focus, focus_radius) in secondary_focuses {
            desired.extend(Self::desired_cells(focus, focus_radius));
        }
        let mut desired = desired.into_iter().collect::<Vec<_>>();
        desired.sort_by_key(|coord| coord.distance_key(center));
        desired
    }
}

fn ordered_residency_offsets() -> &'static Vec<Vec<SceneCellCoord>> {
    static OFFSETS: OnceLock<Vec<Vec<SceneCellCoord>>> = OnceLock::new();
    OFFSETS.get_or_init(|| {
        (0..=SceneStreamingBudget::MAX_RESIDENT_RADIUS)
            .map(|radius| {
                let side = (radius as usize).saturating_mul(2).saturating_add(1);
                let mut offsets = Vec::with_capacity(side.saturating_mul(side));
                for z in -radius..=radius {
                    for x in -radius..=radius {
                        offsets.push(SceneCellCoord { x, z });
                    }
                }
                offsets.sort_by_key(|coord| coord.distance_key(SceneCellCoord { x: 0, z: 0 }));
                offsets
            })
            .collect()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desired_cell_stencil_is_ordered_and_translation_stable() {
        let origin = SceneCellCoord { x: 0, z: 0 };
        let shifted = SceneCellCoord { x: 100, z: -75 };
        let origin_cells = SceneResidencySet::desired_cells(origin, 3);
        let shifted_cells = SceneResidencySet::desired_cells(shifted, 3);
        assert_eq!(origin_cells.len(), 49);
        assert_eq!(shifted_cells.len(), origin_cells.len());
        assert!(origin_cells
            .windows(2)
            .all(|pair| pair[0].distance_key(origin) <= pair[1].distance_key(origin)));
        for (base, moved) in origin_cells.iter().zip(&shifted_cells) {
            assert_eq!(moved.x - shifted.x, base.x);
            assert_eq!(moved.z - shifted.z, base.z);
        }
    }

    #[test]
    fn multi_focus_plan_keeps_primary_scene_and_prefetches_secondary_focus() {
        let center = SceneCellCoord { x: 0, z: 0 };
        let predicted = SceneCellCoord { x: 3, z: 0 };
        let budget = SceneStreamingBudget {
            resident_radius: 1,
            unload_radius: 4,
            max_commits_per_tick: 4,
        };
        let plan = SceneStreamingPlan::build_multi_focus(
            center,
            budget,
            [(predicted, 1)],
            std::iter::empty(),
            std::iter::empty(),
        );

        assert!(plan.desired.contains(&center));
        assert!(plan.desired.contains(&predicted));
        assert_eq!(
            plan.desired.iter().copied().collect::<BTreeSet<_>>().len(),
            plan.desired.len()
        );
    }

    #[test]
    fn predicted_bucket_is_prioritized_ahead_of_regular_far_cells() {
        let center = SceneCellCoord { x: 0, z: 0 };
        let predicted = SceneCellCoord { x: 3, z: 0 };
        let ordinary_far = SceneCellCoord { x: 0, z: 2 };
        let render = [center, ordinary_far, predicted];
        let plan = SceneBucketedCellPlan::from_desired_sets_with_prediction(
            center,
            render,
            render,
            [predicted],
        );

        let predicted_cell = plan
            .cells
            .iter()
            .find(|cell| cell.coord == predicted)
            .unwrap();
        let far_cell = plan
            .cells
            .iter()
            .find(|cell| cell.coord == ordinary_far)
            .unwrap();
        assert_eq!(predicted_cell.bucket, SceneStreamingBucket::PredictedNear);
        assert_eq!(far_cell.bucket, SceneStreamingBucket::VisibleFar);
        assert!(predicted_cell.score > far_cell.score);
    }

    #[test]
    fn custom_layered_plan_preserves_unload_hysteresis_around_primary_center() {
        let center = SceneCellCoord { x: 0, z: 0 };
        let far_loaded = SceneCellCoord { x: -6, z: 0 };
        let profile = SceneStreamingProfile {
            render: SceneStreamingBudget {
                resident_radius: 1,
                unload_radius: 4,
                max_commits_per_tick: 2,
            },
            simulation: SceneStreamingBudget {
                resident_radius: 2,
                unload_radius: 5,
                max_commits_per_tick: 1,
            },
        };
        let render_desired = SceneResidencySet::desired_cells(center, 1);
        let simulation_desired = SceneResidencySet::desired_cells(center, 2);
        let plan = SceneLayeredStreamingPlan::build_from_desired(
            center,
            profile,
            render_desired,
            simulation_desired,
            [far_loaded],
            std::iter::empty(),
            std::iter::empty(),
            std::iter::empty(),
        );
        assert!(plan
            .render
            .unloads
            .iter()
            .any(|request| request.coord == far_loaded));
    }
}
