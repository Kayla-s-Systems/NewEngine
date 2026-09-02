#![forbid(unsafe_op_in_unsafe_fn)]

mod bootstrap;
mod controller;
mod loader;
mod materialize;
mod module;
mod streaming;

pub use bootstrap::AuthoredMapSceneBootstrapProvider;
pub use controller::{
    AuthoredMapCellCoord, AuthoredMapCellDomain, AuthoredMapResidencyPlan,
    AuthoredMapStreamingController, AuthoredMapStreamingDiagnostics, AuthoredMapStreamingFocus,
    AuthoredMapStreamingRuntimeTuning, AuthoredPreparedCellAdmission,
};
pub use loader::{load_authored_definition_entry, load_authored_map_cell, load_authored_map_index};
pub use module::{AuthoredWorldBootstrapCompletion, AuthoredWorldBootstrapModule};
pub use streaming::{
    prepare_authored_map_cell, project_authored_definition_surface,
    AuthoredDefinitionSurfaceBinding, AuthoredMapDefinitionCache, AuthoredMapStreamingSpec,
    AuthoredWorldPlacementSpec, PreparedAuthoredMapCell, WORLD_COLLISION_BOX_PROXY,
    WORLD_COLLISION_PROXY, WORLD_DYNAMIC_PROXY, WORLD_STATIC_PROXY,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AuthoredWorldBootstrapStats {
    pub cells: usize,
    pub placements: usize,
    pub model_actors: usize,
    pub definition_markers: usize,
}

pub type AuthoredDefinitionEntry = newengine_definitions_runtime::DefinitionEntryV1;
