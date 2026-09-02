use std::collections::HashSet;

use super::{
    cell::{SceneCellCoord, SceneStreamingBudget, SceneStreamingProfile},
    residency::SceneResidencySet,
};

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

        // Membership is frame-local and request order is explicitly deterministic,
        // so hash lookup is cheaper than maintaining ordered trees here.
        let loaded_set = loaded.into_iter().collect::<HashSet<_>>();
        let pending_set = pending.into_iter().collect::<HashSet<_>>();
        let desired_set = desired.iter().copied().collect::<HashSet<_>>();

        // `desired` is already ordered by exactly the request priority key. Mapping
        // it preserves that order, so the old second sort of `loads` was redundant.
        let loads = desired
            .iter()
            .copied()
            .filter(|coord| !loaded_set.contains(coord) && !pending_set.contains(coord))
            .map(|coord| SceneStreamingRequest {
                kind: SceneStreamingRequestKind::Load,
                coord,
                priority_key: coord.distance_key(center),
            })
            .collect::<Vec<_>>();

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
        unloads.sort_by_key(|request| std::cmp::Reverse(request.priority_key));

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

    /// Build render/simulation layers from caller-authored desired sets.
    // Render/simulation residency inputs are intentionally parallel at this boundary.
    #[allow(clippy::too_many_arguments)]
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
