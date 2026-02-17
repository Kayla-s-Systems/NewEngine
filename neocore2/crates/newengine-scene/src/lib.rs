#![forbid(unsafe_op_in_unsafe_fn)]
mod bounds;
pub mod components;
mod guid;
mod scene_asset;
mod scene;
mod state;
mod settings;
mod spawn;

pub use bounds::{
    scene_bounds_cached, scene_world_bounds, selection_world_bounds, update_scene_world, SceneBounds,
};
pub use components::{
    ActiveCamera, Controller, EntityGuid, Name, PropertyBag, PropertyValue, SceneRoot,
};
pub use scene::Scene;
pub use scene_asset::{SceneAsset, SceneAssetError, SceneAssetOptions};
pub use guid::{ensure_entity_guid, GuidAllocator};
pub use settings::{ForwardAxis, SceneSettings, UnitScaleMeters, UpAxis};
pub use spawn::{name_or, spawn_named};
pub use state::SceneState;
