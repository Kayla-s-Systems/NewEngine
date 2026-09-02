#![forbid(unsafe_op_in_unsafe_fn)]

mod bootstrap;
mod controller;
mod definition_catalog;
mod loader;
mod materialize;
mod module;
mod scene_admission;
mod streaming;
mod world_runtime;

pub use bootstrap::{
    AuthoredMapSceneBootstrapContributor, AuthoredMapSceneBootstrapProvider,
    ResolvedAuthoredMapBootstrap,
};
pub use controller::{
    AuthoredMapCellCoord, AuthoredMapCellDomain, AuthoredMapResidencyPlan,
    AuthoredMapStreamingController, AuthoredMapStreamingDiagnostics, AuthoredMapStreamingFocus,
    AuthoredMapStreamingRuntimeTuning, AuthoredPreparedCellAdmission,
};
pub use definition_catalog::{
    decode_map_definition_catalog, decode_map_definition_catalog_owned,
    encode_map_definition_catalog, map_definition_catalog_path,
    map_definition_physical_drawable_ref, set_map_definition_physical_drawable_ref,
    MapDefinitionCatalogV1, MAP_DEFINITION_CATALOG_ENCODING_V1, MAP_DEFINITION_CATALOG_SCHEMA_V1,
    MAP_DEFINITION_PHYSICAL_DRAWABLE_METADATA_KEY_V1,
};
pub use loader::{load_authored_definition_entry, load_authored_map_cell, load_authored_map_index};
pub use module::{AuthoredWorldBootstrapCompletion, AuthoredWorldBootstrapModule};
pub use scene_admission::{
    begin_authored_map_streaming, begin_static_world_prefabs, tick_authored_map_streaming,
    tick_authored_static_world_prefabs, AuthoredStaticWorldSpawnSummary,
};
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

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AuthoredGroundPlacementSurface;

pub use world_runtime::{
    install_default_authored_world_streaming_runtime_adapter,
    install_authored_world_streaming_runtime_adapter, AuthoredWorldStreamingRuntimeAdapter,
    AuthoredWorldStreamingRuntimeBinding, AuthoredWorldStreamingWorldRuntimeProvider,
};
