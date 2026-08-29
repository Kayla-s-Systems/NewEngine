#![forbid(unsafe_op_in_unsafe_fn)]

//! Engine-owned population control plane.
//!
//! This crate decides *population policy* only: which ambient subjects should be
//! spawned, retained, retired, or deferred under spatial, CPU and memory pressure.
//! It owns no ECS storage, renderer, AI behavior, pathfinding, physics body or asset
//! provider. Mission/cutscene/script/network ownership is intentionally kept outside
//! ambient eviction accounting.

mod control;
mod model;

pub use control::PopulationControlPlane;
pub use model::{
    PopulationAction, PopulationBands, PopulationCategory, PopulationCategoryBudget,
    PopulationCategoryStats, PopulationControlConfig, PopulationControlPlan,
    PopulationControlStats, PopulationDecisionReason, PopulationFocus, PopulationLocationContext,
    PopulationModelPressure, PopulationOwnership, PopulationPlanEntry, PopulationPressureInput,
    PopulationRegion, PopulationRegionStats, PopulationSubject, MAX_POPULATION_DEBT,
    MAX_POPULATION_GLOBAL_ACTIONS_PER_TICK, POPULATION_PROFILER_SAMPLE_SCHEMA,
};
