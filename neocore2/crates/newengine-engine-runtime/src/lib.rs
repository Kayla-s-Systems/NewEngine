#![forbid(unsafe_op_in_unsafe_fn)]
#![allow(clippy::too_many_arguments)]

//! Engine runtime composition layer.
//!
//! This crate owns reusable runtime systems that are neither game code nor editor
//! UI code: scene bridge, gameplay components/schedules, viewport bridge and the
//! render controller that talks to `newengine-render-api`. Applications consume
//! this layer; they must not create backend resources or Vulkan objects directly.
pub(crate) mod env_config;

pub mod asset_preview;
pub mod audio_gateway;
pub mod authority;
pub mod camera_gateway;
pub mod engine_bounds;
pub mod gameplay;
pub mod input_systems;
pub mod plugin_manager;
pub mod render_controller;
pub mod runtime;
pub mod render_runtime {
    pub use newengine_runtime_host::render_runtime::*;
}
pub mod physics_runtime {
    pub use newengine_runtime_host::physics_runtime::*;
}
pub mod replay {
    pub use newengine_replay::*;
}
mod scene_bootstrap;
pub mod scene_bridge;
mod ui_gateway;
pub mod viewport_bridge;

pub use asset_preview::{AssetPreviewApi, AssetPreviewKind, AssetPreviewSnapshot};
pub use audio_gateway::register_audio_gateway_best_effort;
pub use authority::{
    RuntimeWorldAuthorityBridge, RuntimeWorldAuthorityFrame, RuntimeWorldAuthorityMode,
    RuntimeWorldAuthorityResource,
};

pub use gameplay::{CollisionShapeDesc, GameRunMode, GameplayActor, PhysicsBodyDesc, PlayerActor};
pub use plugin_manager::PluginManagerBridge;
pub use render_controller::RuntimeRenderController;
pub use scene_bridge::SceneBridge;
pub use viewport_bridge::ViewportBridge;
