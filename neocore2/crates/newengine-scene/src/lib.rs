#![forbid(unsafe_op_in_unsafe_fn)]

pub mod components;
mod settings;
mod streaming;

#[cfg(feature = "runtime")]
mod bounds;
#[cfg(feature = "runtime")]
mod guid;
#[cfg(feature = "runtime")]
mod scene;
#[cfg(feature = "runtime")]
mod scene_asset;
#[cfg(feature = "runtime")]
mod spawn;
#[cfg(feature = "runtime")]
mod state;

#[cfg(feature = "runtime")]
pub use bounds::{
    scene_bounds_cached, scene_world_bounds, selection_world_bounds, update_scene_world, SceneBounds,
};
pub use components::{
    ActiveCamera, Controller, EntityGuid, Name, PropertyBag, PropertyValue, SceneRoot,
};
#[cfg(feature = "runtime")]
pub use guid::{ensure_entity_guid, GuidAllocator};
#[cfg(feature = "runtime")]
pub use scene::Scene;
#[cfg(feature = "runtime")]
pub use scene_asset::{SceneAsset, SceneAssetError, SceneAssetOptions};
pub use settings::{ForwardAxis, SceneSettings, UnitScaleMeters, UpAxis};
pub use streaming::{
    SceneBucketedCell, SceneBucketedCellPlan, SceneCellCoord, SceneLayeredStreamingPlan,
    SceneResidencyLayer, SceneResidencySet, SceneStreamingBucket,
    SceneStreamingBudget, SceneStreamingObserver, SceneStreamingPlan, SceneStreamingProfile,
    SceneStreamingRequest, SceneStreamingRequestKind,
};
#[cfg(feature = "runtime")]
pub use spawn::{name_or, spawn_named};
#[cfg(feature = "runtime")]
pub use state::SceneState;
