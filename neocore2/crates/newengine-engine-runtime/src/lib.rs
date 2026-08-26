#![forbid(unsafe_op_in_unsafe_fn)]
#![allow(clippy::too_many_arguments)]

//! Engine runtime composition layer.
//!
//! This crate owns reusable runtime systems that are neither game code nor editor
//! UI code: scene bridge, gameplay components/schedules, viewport bridge and the
//! render controller that talks to `newengine-render-api`. Applications consume
//! this layer; they must not create backend resources or backend objects directly.
mod editor_viewport;
mod env_config;
mod provider_contract;
mod runtime_composition;
mod runtime_policy;
pub mod world_authoring;
mod world_runtime_provider;

pub mod asset_preview;
pub mod audio_ambience;
pub mod audio_environment;
pub mod audio_gateway;
pub mod audio_occlusion;
pub mod audio_scene;
pub mod authority;
pub mod camera_gateway;
pub mod engine_bounds;
pub mod gameplay;
pub mod input_systems;
pub mod plugin_manager;
pub mod render_controller;
pub mod runtime;
pub mod render_runtime {
    pub use newengine_render_runtime_adapter::*;
}
pub mod physics_runtime {
    pub use newengine_physics_runtime_adapter::*;
}
pub mod replay {
    pub use newengine_replay::*;
}
mod scene_bootstrap;
pub mod scene_bridge;
mod ui_gateway;
pub mod viewport_bridge;

pub use asset_preview::{AssetPreviewApi, AssetPreviewKind, AssetPreviewSnapshot};
pub use audio_ambience::{AudioAmbienceBedRuntime, AudioAmbienceRuntimeModule};
pub use audio_environment::{
    AudioEnvironmentFrame, AudioEnvironmentResolution, AudioEnvironmentRuntimeState,
};
pub use audio_gateway::register_audio_gateway_best_effort;
pub use audio_occlusion::{
    acoustic_material_profile_for_surface, AudioListenerRuntimeState, AudioOcclusionObservation,
    AudioOcclusionPhysicsQueryProvider,
};
pub use audio_scene::{
    AcousticSurface, AudioEmitter, AudioEmitterRuntime, AudioEnvironmentZone, AudioPortal,
    AudioSceneRuntimeModule,
};
pub use authority::{
    RuntimeWorldAuthorityBridge, RuntimeWorldAuthorityFrame, RuntimeWorldAuthorityMode,
    RuntimeWorldAuthorityResource,
};
pub use newengine_audio_api::AudioAmbienceBed;

pub use gameplay::{CollisionShapeDesc, GameRunMode, GameplayActor, PhysicsBodyDesc, PlayerActor};
pub use plugin_manager::PluginManagerBridge;
pub use provider_contract::{
    validate_provider_contract, RuntimeProviderDescriptor, I_GAMEPLAY_CONTENT_PROVIDER_V1,
    I_GAMEPLAY_PHYSICS_QUERY_PROVIDER_V1, I_GAMEPLAY_SYSTEM_PROVIDER_V1, I_GAMEPLAY_UI_PROVIDER_V1,
    I_SCENE_BOOTSTRAP_PROVIDER_V1, I_WORLD_RUNTIME_PROVIDER_V1, PROVIDER_CONTRACT_V1,
};
pub use render_controller::RuntimeRenderController;
pub use runtime_composition::RuntimeRenderContributionRegistry;
pub use scene_bridge::{
    SceneBootstrapContext, SceneBootstrapProvider, SceneBootstrapResult, SceneBridge,
};
pub use viewport_bridge::ViewportBridge;
pub use world_runtime_provider::{
    WorldRuntimeFrame, WorldRuntimeProvider, WorldRuntimeProviderRegistry,
};
