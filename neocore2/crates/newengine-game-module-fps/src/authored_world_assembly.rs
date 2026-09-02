#![forbid(unsafe_op_in_unsafe_fn)]
#![allow(clippy::too_many_arguments)]

use newengine_gameplay_fps_api::{
    FpsMotionResponseTuning, FpsObjectiveState, FpsObjectiveTarget, FpsPlayerTuning,
    FpsRuntimeRules,
};

mod assets_bootstrap;

mod runtime_contributions;

use newengine_assets::{wait_ready, AssetServiceClient};
use newengine_ecs::EntityId;
use newengine_materials::{MaterialId, MaterialRegistry};
use newengine_math::{EulerRot, Quat, Vec3};
use newengine_plugin_host::default_host_api;
use newengine_primitives::{
    Primitive, PrimitiveId, PrimitiveMesh, PrimitiveRegistry, PrimitiveVertex,
};
use newengine_scene::{spawn_named, Scene};
use newengine_transform::{set_parent, Transform};

use newengine_engine_runtime::gameplay::{spawn_player_controller, WorldActivationState};
use newengine_engine_runtime::world_authoring::bootstrap_runtime_scene_foundation;

use newengine_engine_runtime::world_authoring::{
    ensure_scene_root as ensure_root, validate_scene_objects,
};
use newengine_fps_content_runtime::authored_world_profile::{
    load_authored_world_profile, load_authored_world_profile_from_resolved_map,
    AuthoredFpsAudioEmitterSpec, AuthoredFpsDayNightSpec, AuthoredFpsDefinitionApplyMode,
    AuthoredFpsDefinitionInstanceSpec, AuthoredFpsGameplaySpec, AuthoredFpsLightingSpec,
    AuthoredFpsShadowSpec, AuthoredFpsSkyAtmosphereSpec, AuthoredFpsSkySpec,
    AuthoredFpsTerrainSpec, AuthoredWorldProfile,
};

use self::assets_bootstrap::bootstrap_authored_fps_world_scene_with_resolved_map_impl;

#[inline]
pub(crate) fn character_body_from_fps_tuning(
    tuning: FpsPlayerTuning,
) -> newengine_engine_runtime::gameplay::CharacterBody {
    let value = tuning.sanitized();
    newengine_engine_runtime::gameplay::CharacterBody {
        radius: value.body_radius,
        standing_half_height: value.body_half_height,
        crouched_half_height: value.crouched_body_half_height,
        standing_eye_height: value.camera_eye_height,
        crouched_eye_height: value.crouched_camera_eye_height,
        visual_radius: value.visual_radius,
        visual_half_height: value.visual_half_height,
    }
    .sanitized()
}

#[inline]
pub(crate) fn character_motion_from_fps_tuning(
    tuning: FpsPlayerTuning,
) -> newengine_engine_runtime::gameplay::CharacterMotionTuning {
    let value = tuning.sanitized();
    newengine_engine_runtime::gameplay::CharacterMotionTuning {
        sprint_multiplier: value.sprint_multiplier,
        jump_speed: value.jump_speed,
        stance_camera_speed: value.crouch_camera_speed,
    }
    .sanitized()
}

/// Transitional domain assembly contributor entrypoint. The generic authored-world bootstrap
/// provider already resolved startup_scene and the authoritative map INDEX; FPS authored consumes
/// that resolved context without owning SceneBootstrapProvider identity or repeating INDEX RPC.
pub fn bootstrap_authored_fps_scene_with_resolved_map(
    scene: &mut Scene,
    primitives: &mut PrimitiveRegistry,
    materials: &MaterialRegistry,
    resolved_map: &newengine_authored_world_runtime::ResolvedAuthoredMapBootstrap,
) -> Option<EntityId> {
    bootstrap_authored_fps_world_scene_with_resolved_map_impl(
        scene,
        primitives,
        materials,
        resolved_map,
    )
}
