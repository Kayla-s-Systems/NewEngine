#![forbid(unsafe_op_in_unsafe_fn)]
#![allow(clippy::too_many_arguments)]

//! Engine runtime composition layer.
//!
//! This crate owns reusable runtime systems that are neither game code nor editor
//! UI code: scene bridge, gameplay components/schedules and the
//! render controller that talks to `newengine-render-api`. Applications consume
//! this layer; they must not create backend resources or backend objects directly.
mod editor_viewport_adapter;
mod env_config;
mod provider_contract;
mod runtime_composition;
mod runtime_policy;
pub mod world_authoring;
mod world_runtime_provider;

pub mod audio_diffraction;
pub mod audio_occlusion;
pub mod audio_reflections;
pub mod authority;
pub mod camera_gateway;
pub mod engine_bounds;
pub mod gameplay;
pub mod input_systems {
    pub use newengine_input_systems_runtime::*;
}
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
pub use audio_diffraction::AudioDiffractionPhysicsQueryProvider;
pub use audio_occlusion::AudioOcclusionPhysicsQueryProvider;
pub use audio_reflections::AudioReflectionPhysicsQueryProvider;
pub use authority::{
    RuntimeWorldAuthorityBridge, RuntimeWorldAuthorityFrame, RuntimeWorldAuthorityMode,
    RuntimeWorldAuthorityResource,
};
pub use newengine_audio_api::{
    AcousticSurface, AudioAmbienceBed, AudioEmitter, AudioEnvironmentZone, AudioPortal,
};
pub use newengine_audio_world_api::{
    AudioAmbienceBedRuntime, AudioEarlyReflectionObservation, AudioEarlyReflectionPathObservation,
    AudioEdgeDiffractionObservation, AudioEdgeDiffractionPathObservation, AudioEmitterRuntime,
    AudioListenerRuntimeState, AudioOcclusionObservation,
};

pub use gameplay::{CollisionShapeDesc, GameRunMode, GameplayActor, PhysicsBodyDesc, PlayerActor};
pub use newengine_viewport_bridge::ViewportBridge;
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
pub use world_runtime_provider::{
    WorldRuntimeFrame, WorldRuntimeProvider, WorldRuntimeProviderRegistry,
};
