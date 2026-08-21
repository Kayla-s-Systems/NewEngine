#![forbid(unsafe_op_in_unsafe_fn)]
#![allow(clippy::too_many_arguments)]

use newengine_game_data::GameDataSnapshot;
use newengine_gameplay_fps_api::{
    FpsDemoGoal, FpsDemoHazard, FpsDemoPickup, FpsDemoRules, FpsDemoState, FpsDemoTarget,
    FpsPlayerTuning,
};

#[path = "game_ready_parts/assets_bootstrap.rs"]
mod assets_bootstrap;
mod content;
mod env_config;
#[path = "game_ready_parts/foliage.rs"]
mod foliage;
#[path = "game_ready_parts/material_source.rs"]
mod material_source;
#[path = "game_ready_parts/materials_terrain.rs"]
mod materials_terrain;
#[path = "game_ready_parts/mission.rs"]
mod mission;
#[path = "game_ready_parts/player_model.rs"]
mod player_model;
#[path = "game_ready_parts/shadow_torture.rs"]
mod shadow_torture;
#[path = "game_ready_parts/sky.rs"]
mod sky;
#[path = "game_ready_parts/terrain_heightmap.rs"]
mod terrain_heightmap;
#[path = "game_ready_parts/terrain_streaming.rs"]
mod terrain_streaming;
#[path = "game_ready_parts/world_model.rs"]
mod world_model;
#[path = "game_ready_parts/ytyp_metadata.rs"]
mod ytyp_metadata;

use core::f32::consts::TAU;

use newengine_assets::{wait_ready, AssetServiceClient};
use newengine_bounds::Bounds;
use newengine_core::{
    call_service_v1_optional, TaskLane, TaskPriority, TaskRequest, TaskTicket, ThreadPoolHandle,
};
use newengine_ecs::EntityId;
use newengine_lighting::{AmbientLight, DirectionalLight, ShadowSettings};
use newengine_materials::{MaterialFlags, MaterialId, MaterialRegistry};
use newengine_math::{EulerRot, Quat, Vec3};
use newengine_plugin_host::default_host_api;
use newengine_primitives::{
    builtins, fnv1a_64, Primitive, PrimitiveId, PrimitiveMesh, PrimitiveRegistry, PrimitiveVertex,
};
use newengine_procedural_noise::{
    DomainWarp2D, NoiseAlgorithm, NoiseCombineMode, NoiseDomain2D, NoiseGraph2D, NoiseLayer2D,
    NoiseRemap, NoiseShape, ProceduralTerrain, TerrainHeightfieldDescriptor,
};
use newengine_scene::{
    spawn_named, Scene, SceneBucketedCellPlan, SceneCellCoord, SceneLayeredStreamingPlan,
    SceneResidencySet, SceneStreamingBudget, SceneStreamingProfile,
};
use newengine_task_api::{task_domain, task_pass};
use newengine_transform::{set_parent, Transform};

use std::sync::{Arc, Mutex};

use newengine_engine_runtime::gameplay::{spawn_player_controller, WorldActivationState};
use newengine_engine_runtime::world_authoring::bootstrap_runtime_scene;

use self::content::{
    load_game_ready_map_profile, GameReadyDayNightSpec, GameReadyDefinitionApplyMode,
    GameReadyDefinitionInstanceSpec, GameReadyFoliageSpec, GameReadyGameplaySpec,
    GameReadyLightingSpec, GameReadyMapProfile, GameReadyMaterialSetSpec, GameReadyMaterialSpec,
    GameReadyMissionSpec, GameReadyPaletteSpec, GameReadyPrefabSpec, GameReadyShadowSpec,
    GameReadySkyAtmosphereSpec, GameReadySkySpec, GameReadyTerrainHeightmapSpec,
    GameReadyTerrainSpec,
};
use newengine_engine_runtime::world_authoring::{
    apply_exact_material, apply_primitive_material_instance as apply_primitive_instance,
    ensure_primitive_material_base as ensure_primitive_base, ensure_scene_root as ensure_root,
    primitive_bounds, validate_scene_objects,
};

use self::assets_bootstrap::bootstrap_game_ready_world_scene_impl;

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

use self::sky::{tick_game_ready_sky_cycle, SkyVisualKind};
use self::terrain_streaming::tick_game_ready_streaming_terrain;
use self::world_model::tick_game_ready_static_world_prefabs;

use self::material_source::*;
use self::materials_terrain::*;
use self::sky::*;

/// Assemble the authored GameReady world through the product-owned world package.
pub fn bootstrap_world_scene(
    scene: &mut Scene,
    primitives: &mut PrimitiveRegistry,
    materials: &MaterialRegistry,
) -> Option<EntityId> {
    bootstrap_world_scene_with_data(
        scene,
        primitives,
        materials,
        GameDataSnapshot::rust_defaults(),
    )
}

/// Assemble the authored world using an immutable provider-produced data snapshot.
/// This is the Lua-ready entrypoint: the world package does not care who produced the data.
pub fn bootstrap_world_scene_with_data(
    scene: &mut Scene,
    primitives: &mut PrimitiveRegistry,
    materials: &MaterialRegistry,
    game_data: GameDataSnapshot,
) -> Option<EntityId> {
    bootstrap_game_ready_world_scene_impl(scene, primitives, materials, game_data)
}

/// Progress launch-blocking world assembly.
pub fn tick_prelaunch(
    world: &mut newengine_ecs::World,
    primitives: &mut PrimitiveRegistry,
    materials: &MaterialRegistry,
    thread_pool: Option<&ThreadPoolHandle>,
) {
    tick_game_ready_static_world_prefabs(world, primitives, materials, thread_pool);
}

/// Progress normal GameReady world streaming/environment work.
pub fn tick_frame(
    world: &mut newengine_ecs::World,
    primitives: &mut PrimitiveRegistry,
    materials: &MaterialRegistry,
    thread_pool: Option<&ThreadPoolHandle>,
    frame: newengine_engine_runtime::WorldRuntimeFrame,
) {
    player_model::tick_player_model_assignments(world, primitives, materials);
    player_model::tick_player_skin_animation(world, frame.dt);
    tick_game_ready_static_world_prefabs(world, primitives, materials, thread_pool);
    if frame.runtime_active && frame.streaming_enabled {
        tick_game_ready_streaming_terrain(world, materials, thread_pool);
    }
    if frame.environment_cycle_enabled {
        tick_game_ready_sky_cycle(world, frame.dt);
    }
    shadow_torture::tick(world, frame.dt);
}
