#![forbid(unsafe_op_in_unsafe_fn)]
#![allow(clippy::too_many_arguments)]

use newengine_game_data::GameDataSnapshot;
use newengine_gameplay_fps_api::{
    FpsMotionResponseTuning, FpsObjectiveGoal, FpsObjectiveHazard, FpsObjectivePickup,
    FpsObjectiveState, FpsObjectiveTarget, FpsPlayerTuning, FpsRuntimeRules,
};

#[path = "game_ready_parts/animation_events.rs"]
mod animation_events;
#[path = "game_ready_parts/animation_semantic.rs"]
mod animation_semantic;
#[path = "game_ready_parts/assets_bootstrap.rs"]
mod assets_bootstrap;
mod content;
mod env_config;
#[path = "game_ready_parts/equipment_visual.rs"]
mod equipment_visual;
#[path = "game_ready_parts/foliage.rs"]
mod foliage;
#[path = "game_ready_parts/impact_debris.rs"]
mod impact_debris;
#[path = "game_ready_parts/material_source.rs"]
mod material_source;
#[path = "game_ready_parts/materials_terrain.rs"]
mod materials_terrain;
#[path = "game_ready_parts/mission.rs"]
mod mission;
#[path = "game_ready_parts/player_hair.rs"]
mod player_hair;
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
#[path = "game_ready_parts/vfx_decal_materials.rs"]
mod vfx_decal_materials;
#[path = "game_ready_parts/weapon_animation.rs"]
mod weapon_animation;
#[path = "game_ready_parts/weapon_casing.rs"]
mod weapon_casing;
#[path = "game_ready_parts/weapon_grip.rs"]
mod weapon_grip;
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
    SceneResidencySet, SceneStreamingBudget, SceneStreamingObserver, SceneStreamingProfile,
};
use newengine_task_api::{task_domain, task_pass};
use newengine_transform::{set_parent, Transform};

use std::sync::{Arc, Mutex};

use newengine_engine_runtime::gameplay::{spawn_player_controller, WorldActivationState};
use newengine_engine_runtime::world_authoring::bootstrap_runtime_scene_foundation;

use self::content::{
    load_authored_world_profile, AuthoredMissionSpec, AuthoredWorldProfile,
    GameReadyAudioEmitterSpec, GameReadyDayNightSpec, GameReadyDefinitionApplyMode,
    GameReadyDefinitionInstanceSpec, GameReadyFoliageSpec, GameReadyGameplaySpec,
    GameReadyLightingSpec, GameReadyMaterialSetSpec, GameReadyMaterialSpec, GameReadyPaletteSpec,
    AuthoredWorldPlacementSpec, GameReadyShadowSpec, GameReadySkyAtmosphereSpec, GameReadySkySpec,
    GameReadyTerrainHeightmapSpec, GameReadyTerrainSpec,
};
use newengine_engine_runtime::world_authoring::{
    apply_exact_material, apply_primitive_material_instance as apply_primitive_instance,
    ensure_primitive_material_base as ensure_primitive_base, ensure_scene_root as ensure_root,
    primitive_bounds, validate_scene_objects,
};

use self::assets_bootstrap::bootstrap_game_ready_world_scene_impl;
pub use player_hair::{
    bind_compiled_player_groom_v1, bind_player_nehair_v1, install_nehair_groom_v1,
    load_nehair_groom_v1,
};

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

use self::foliage::tick_deferred_foliage_prefabs;
use self::mission::{tick_deferred_item_pickups, tick_runtime_world_item_visuals};
use self::sky::{tick_game_ready_sky_cycle, SkyVisualKind};
use self::terrain_streaming::{tick_game_ready_streaming_terrain, TerrainSurfaceSampler};
use self::world_model::{tick_authored_map_streaming, tick_game_ready_static_world_prefabs};

use self::material_source::*;
use self::materials_terrain::*;
use self::sky::*;

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
    tick_authored_map_streaming(world, primitives, materials, thread_pool);
    tick_game_ready_static_world_prefabs(world, primitives, materials, thread_pool);
    tick_deferred_foliage_prefabs(world, primitives, materials);
    tick_deferred_item_pickups(world, primitives, materials);
    tick_runtime_world_item_visuals(world, primitives, materials);
}

#[derive(Clone, Copy, Debug, Default)]
struct GameReadyFrameTiming {
    model_assign_ms: f32,
    model_ground_ms: f32,
    weapon_input_ms: f32,
    anim_semantic_ms: f32,
    fpp_anchor_ms: f32,
    skin_animation_ms: f32,
    skin_sidecars_ms: f32,
    weapon_visual_ms: f32,
    weapon_casing_ms: f32,
    impact_debris_ms: f32,
    weapon_animation_ms: f32,
    map_streaming_ms: f32,
    static_world_ms: f32,
    foliage_ms: f32,
    item_pickups_ms: f32,
    world_items_ms: f32,
    terrain_streaming_ms: f32,
    sky_ms: f32,
    shadow_torture_ms: f32,
}

#[inline]
fn should_emit_game_ready_frame_profile(frame_index: u64, total_ms: f32) -> bool {
    total_ms >= 4.0 || frame_index.is_multiple_of(120)
}

#[inline]
fn emit_game_ready_frame_profile(frame_index: u64, total_ms: f32, timing: GameReadyFrameTiming) {
    let payload = serde_json::json!({
        "schema": "newengine.diagnostics.profiler.sample.v1",
        "category": "world.runtime",
        "source": "newengine-game-ready-world",
        "name": "game-ready world runtime frame",
        "lane": "world-runtime",
        "priority": "interactive",
        "dependency_group": format!("world.runtime.frame.{frame_index}"),
        "frame_index": frame_index,
        "elapsed_ms": total_ms,
        "budget_ms": 4.0,
        "slow": total_ms >= 4.0,
        "model_assign_ms": timing.model_assign_ms,
        "model_ground_ms": timing.model_ground_ms,
        "weapon_input_ms": timing.weapon_input_ms,
        "anim_semantic_ms": timing.anim_semantic_ms,
        "fpp_anchor_ms": timing.fpp_anchor_ms,
        "skin_animation_ms": timing.skin_animation_ms,
        "skin_sidecars_ms": timing.skin_sidecars_ms,
        "weapon_visual_ms": timing.weapon_visual_ms,
        "weapon_casing_ms": timing.weapon_casing_ms,
        "impact_debris_ms": timing.impact_debris_ms,
        "weapon_animation_ms": timing.weapon_animation_ms,
        "map_streaming_ms": timing.map_streaming_ms,
        "static_world_ms": timing.static_world_ms,
        "foliage_ms": timing.foliage_ms,
        "item_pickups_ms": timing.item_pickups_ms,
        "world_items_ms": timing.world_items_ms,
        "terrain_streaming_ms": timing.terrain_streaming_ms,
        "sky_ms": timing.sky_ms,
        "shadow_torture_ms": timing.shadow_torture_ms,
    });
    if let Ok(bytes) = serde_json::to_vec(&payload) {
        let _ = newengine_plugin_host::emit_plugin_event(
            "newengine.diagnostics.profiler.sample.v1",
            &bytes,
        );
    }
}

/// Progress normal GameReady world streaming/environment work.
pub fn tick_frame(
    world: &mut newengine_ecs::World,
    primitives: &mut PrimitiveRegistry,
    materials: &MaterialRegistry,
    thread_pool: Option<&ThreadPoolHandle>,
    frame: newengine_engine_runtime::WorldRuntimeFrame,
) {
    let frame_started = std::time::Instant::now();
    let mut phase_started = frame_started;
    let mut timing = GameReadyFrameTiming::default();

    macro_rules! phase_done {
        ($field:ident) => {{
            let now = std::time::Instant::now();
            timing.$field = now.duration_since(phase_started).as_secs_f32() * 1000.0;
            phase_started = now;
        }};
    }

    player_model::tick_player_model_assignments(world, primitives, materials);
    phase_done!(model_assign_ms);
    player_model::tick_player_model_grounding(world);
    phase_done!(model_ground_ms);
    equipment_visual::tick_equipped_weapon_presentation_input(world, frame.dt);
    phase_done!(weapon_input_ms);
    animation_semantic::capture_animation_semantic_frame(world);
    phase_done!(anim_semantic_ms);
    // The stable FPP eye anchor is actor/stance-owned and independent from animated head joints.
    // Publish it before arm/weapon animation so camera and FPP grip solve consume one frame authority.
    player_model::publish_player_first_person_camera_anchors(world);
    phase_done!(fpp_anchor_ms);
    player_model::tick_player_skin_animation(world, frame.dt, frame.frame_index);
    phase_done!(skin_animation_ms);
    player_model::tick_player_skin_sidecars(world);
    phase_done!(skin_sidecars_ms);
    equipment_visual::tick_equipped_weapon_visuals(world, primitives, materials, frame.dt);
    phase_done!(weapon_visual_ms);
    weapon_casing::tick_weapon_shell_casing_visuals(world, primitives, materials);
    phase_done!(weapon_casing_ms);
    impact_debris::tick_persistent_impact_debris_visuals(world, primitives, materials);
    phase_done!(impact_debris_ms);
    weapon_animation::tick_equipped_weapon_animations(world, frame.dt);
    phase_done!(weapon_animation_ms);
    vfx_decal_materials::tick_vfx_decal_material_bindings(world, materials);
    if frame.runtime_active && frame.streaming_enabled {
        tick_authored_map_streaming(world, primitives, materials, thread_pool);
    }
    phase_done!(map_streaming_ms);
    tick_game_ready_static_world_prefabs(world, primitives, materials, thread_pool);
    phase_done!(static_world_ms);
    tick_deferred_foliage_prefabs(world, primitives, materials);
    phase_done!(foliage_ms);
    tick_deferred_item_pickups(world, primitives, materials);
    phase_done!(item_pickups_ms);
    tick_runtime_world_item_visuals(world, primitives, materials);
    phase_done!(world_items_ms);
    if frame.runtime_active && frame.streaming_enabled {
        tick_game_ready_streaming_terrain(world, materials, thread_pool);
    }
    phase_done!(terrain_streaming_ms);
    if frame.environment_cycle_enabled {
        tick_game_ready_sky_cycle(world, frame.dt);
    }
    phase_done!(sky_ms);
    shadow_torture::tick(world, frame.dt);
    timing.shadow_torture_ms = phase_started.elapsed().as_secs_f32() * 1000.0;

    let total_ms = frame_started.elapsed().as_secs_f32() * 1000.0;
    if should_emit_game_ready_frame_profile(frame.frame_index, total_ms) {
        emit_game_ready_frame_profile(frame.frame_index, total_ms, timing);
    }
}

#[cfg(test)]
mod game_ready_frame_profile_policy_tests {
    use super::should_emit_game_ready_frame_profile;

    #[test]
    fn slow_world_runtime_frame_is_always_profiled() {
        assert!(should_emit_game_ready_frame_profile(7, 4.0));
        assert!(should_emit_game_ready_frame_profile(7, 38.7));
    }

    #[test]
    fn fast_world_runtime_frame_is_sampled_only_periodically() {
        assert!(!should_emit_game_ready_frame_profile(119, 3.99));
        assert!(should_emit_game_ready_frame_profile(120, 0.25));
    }
}
