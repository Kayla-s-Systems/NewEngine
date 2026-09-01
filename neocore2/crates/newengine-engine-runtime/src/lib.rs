#![forbid(unsafe_op_in_unsafe_fn)]
#![allow(clippy::too_many_arguments)]

//! Thin engine runtime composition and compatibility facade.
//!
//! Domain implementations live in dedicated capability/runtime crates. This crate owns only
//! cross-domain composition plus stable compatibility re-exports for existing applications.
//! New domain semantics must not be implemented here.
mod editor_viewport_adapter {}
mod env_config {}
mod provider_contract {
    pub use newengine_runtime_provider_api::*;
}
mod runtime_composition;
pub mod world_authoring {
    pub use newengine_scene_bridge_runtime::world_authoring::*;
}
mod world_runtime_provider {
    pub use newengine_world_runtime_api::*;
}

pub mod audio_diffraction {
    pub use newengine_audio_world_runtime::audio_diffraction::*;
}
pub mod audio_occlusion {
    pub use newengine_audio_world_runtime::audio_occlusion::*;
}
pub mod audio_reflections {
    pub use newengine_audio_world_runtime::audio_reflections::*;
}
pub mod authority {
    pub use newengine_world_authority_runtime::{
        current_entity_authority_map, current_world_authority_frame, RuntimeEntityAuthorityMap,
        RuntimeWorldAuthorityBridge, RuntimeWorldAuthorityFrame, RuntimeWorldAuthorityMode,
        RuntimeWorldAuthorityResource,
    };
}
pub mod camera_gateway {
    pub use newengine_camera_gateway_runtime::*;
}
pub mod engine_bounds {
    pub use newengine_bounds::EngineBoundsSnap;
}
pub mod gameplay {
    pub use newengine_gameplay_world_runtime::gameplay::*;
}
pub mod input_systems {
    pub use newengine_input_systems_runtime::*;
}
pub mod plugin_manager {
    pub use newengine_plugin_manager_bridge::PluginManagerBridge;
}
pub mod render_controller {
    pub use newengine_render_world_runtime::render_controller::*;
}
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

pub mod scene_bridge {
    pub use newengine_scene_bridge_runtime::scene_bridge::*;
}
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
pub use newengine_audio_world_runtime::{
    AudioDiffractionPhysicsQueryProvider, AudioOcclusionPhysicsQueryProvider,
    AudioReflectionPhysicsQueryProvider,
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
