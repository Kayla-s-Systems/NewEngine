#![forbid(unsafe_op_in_unsafe_fn)]
mod bounds;
mod components;
mod scene;
mod state;
mod settings;
mod spawn;

pub use bounds::{
    scene_world_bounds, selection_world_bounds, update_scene_world, SceneBounds,
};
pub use components::{
    ActiveCamera, Controller, Name, PropertyBag, PropertyValue, SceneRoot,
};
pub use scene::Scene;
pub use settings::{ForwardAxis, SceneSettings, UnitScaleMeters, UpAxis};
pub use spawn::{name_or, spawn_named};
pub use state::SceneState;
