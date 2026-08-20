use super::foliage::{spawn_foliage_prefabs, terrain_height, SKYDOME_PRIMITIVE_ID};
use super::materials_terrain::register_demo_materials;
use super::mission::spawn_game_ready_mission;
use super::player_model::spawn_game_ready_player_model;
use super::sky::configure_game_ready_lighting;
use super::terrain_streaming::spawn_procedural_terrain;
use super::world_model::begin_static_world_prefabs;
use super::ytyp_metadata::{apply_game_ready_ytyp_metadata, resolve_game_ready_asset_graph};
use super::*;
use newengine_game_data::{GameData, GameDataSnapshot};

use self::mesh_assets::ensure_skydome_primitive;

mod mesh_assets;

#[path = "assets_bootstrap_definitions.rs"]
mod definitions;
#[path = "assets_bootstrap_layout.rs"]
mod layout;
#[path = "assets_bootstrap_rules.rs"]
mod rules;
#[path = "assets_bootstrap_sky.rs"]
mod sky_visual;

use definitions::instantiate_game_ready_definitions;
use layout::{spawn_authored_terrain_reference, spawn_game_ready_scene_entity_layout};
use rules::to_fps_demo_rules;
use sky_visual::spawn_skydome;

#[inline]
fn game_data_color(value: [f32; 3], fallback: [f32; 3]) -> [f32; 3] {
    let mut out = value;
    for (channel, fallback_channel) in out.iter_mut().zip(fallback) {
        if !channel.is_finite() {
            *channel = fallback_channel;
        }
        *channel = channel.clamp(0.0, 1.0);
    }
    out
}

#[inline]
fn game_data_direction(value: [f32; 3], fallback: [f32; 3]) -> [f32; 3] {
    let candidate = Vec3::new(value[0], value[1], value[2]);
    let direction = if candidate.is_finite() && candidate.length_squared() > 1.0e-6 {
        candidate.normalize_or_zero()
    } else {
        Vec3::new(fallback[0], fallback[1], fallback[2]).normalize_or_zero()
    };
    [direction.x, direction.y, direction.z]
}

#[inline]
fn game_data_shadow_filter(value: &str) -> newengine_lighting::ShadowFilter {
    match value.trim().to_ascii_lowercase().as_str() {
        "hard" | "none" => newengine_lighting::ShadowFilter::Hard,
        "pcf" => newengine_lighting::ShadowFilter::Pcf,
        "pcss" => newengine_lighting::ShadowFilter::Pcss,
        _ => newengine_lighting::ShadowFilter::Pcf,
    }
}

/// Provider-produced GameData is authoritative for runtime lighting policy.
/// YMAP/YTYP still owns scene content/metadata, but built-in GameReady defaults
/// must not silently replace project shadow/day-night settings.
#[inline]
fn game_data_lighting_spec(data: &GameData) -> GameReadyLightingSpec {
    let lighting = &data.world.lighting;
    let shadows = &data.world.shadows;
    let day_night = data.world.day_night;
    GameReadyLightingSpec {
        ambient_color: game_data_color(lighting.ambient_color, [0.42, 0.47, 0.56]),
        ambient_intensity: lighting.ambient_intensity.clamp(0.0, 8.0),
        sun_direction: game_data_direction(lighting.sun_direction, [-0.55, -0.82, -0.28]),
        sun_color: game_data_color(lighting.sun_color, [1.0, 0.955, 0.86]),
        sun_intensity: lighting.sun_intensity.clamp(0.0, 32.0),
        shadows: GameReadyShadowSpec {
            enabled: shadows.enabled,
            resolution: shadows.resolution.clamp(256, 8192),
            cascade_count: shadows.cascade_count.clamp(1, 4),
            max_distance: shadows.max_distance.clamp(1.0, 1000.0),
            softness: shadows.softness.clamp(0.0, 16.0),
            bias: shadows.bias.clamp(0.0, 0.1),
            normal_bias: shadows.normal_bias.clamp(0.0, 0.5),
            contact_strength: shadows.contact_strength.clamp(0.0, 1.0),
            filter: game_data_shadow_filter(&shadows.filter),
            pcss: newengine_lighting::ShadowPcssSettings {
                light_angular_radius_degrees: shadows.pcss_light_angular_radius_degrees,
                blocker_search_radius_texels: shadows.pcss_blocker_search_radius_texels,
                max_filter_radius_texels: shadows.pcss_max_filter_radius_texels,
                blocker_samples: shadows.pcss_blocker_samples,
                filter_samples: shadows.pcss_filter_samples,
                min_filter_radius_texels: shadows.pcss_min_filter_radius_texels,
                stable_kernel_cell_texels: shadows.pcss_stable_kernel_cell_texels,
            }
            .sanitized(),
        },
        day_night: GameReadyDayNightSpec {
            enabled: day_night.enabled,
            time_of_day_hours: day_night.time_of_day_hours.rem_euclid(24.0),
            day_length_seconds: day_night.day_length_seconds.clamp(30.0, 86_400.0),
            day_of_year: day_night.day_of_year.clamp(1, 366),
            latitude_degrees: day_night.latitude_degrees.clamp(-89.0, 89.0),
            axial_tilt_degrees: day_night.axial_tilt_degrees.clamp(-45.0, 45.0),
        },
    }
}

pub(crate) fn bootstrap_game_ready_world_scene_impl(
    scene: &mut Scene,
    prims: &mut PrimitiveRegistry,
    mats: &MaterialRegistry,
    game_data: GameDataSnapshot,
) -> Option<EntityId> {
    *scene = Scene::new();
    bootstrap_runtime_scene(scene);

    let root = ensure_root(scene);
    let active_camera = scene.active_camera();
    let mut map = match load_game_ready_map_profile() {
        Ok(map) => map,
        Err(errors) => {
            newengine_ulog_api::ulog::error!(
                "game-ready: strict data-driven scene bootstrap failed; authored .ymap was not resolved into a valid XML map profile; emergency fallback profiles are forbidden; attempts='{}'",
                errors.join(" | ")
            );
            newengine_core::crash::record_breadcrumb(format!(
                "game-ready scene bootstrap failed: authored .ymap unresolved attempts={}",
                errors.join(" | ")
            ));
            return None;
        }
    };
    apply_game_ready_ytyp_metadata(&mut map);
    // Apply project/provider data before any light or sky entity is constructed.
    map.lighting = game_data_lighting_spec(game_data.data());
    let materials = register_demo_materials(mats, &map.palette, &map.materials);
    let world = scene.world_mut();
    newengine_ulog_api::ulog::info!(
        "game-ready game-data snapshot installed source='{}' schema='{}' version={}",
        game_data.source_id(),
        game_data.data().schema,
        game_data.data().version,
    );
    world.insert_resource(game_data.clone());

    let rules = to_fps_demo_rules(&map.gameplay, &map.player.model, game_data.data());
    world.insert_resource(rules.clone());
    world.insert_resource(WorldActivationState::new(
        "waiting for CPU scene assembly and GPU material residency",
    ));

    let layout = spawn_game_ready_scene_entity_layout(world, root);

    configure_game_ready_lighting(world, layout.environment, &map.lighting);

    let initial_terrain_center = newengine_scene::SceneCellCoord::from_world_pos(
        map.player.start,
        map.terrain.size_x,
        map.terrain.size_z,
    );
    let terrain = if map.terrain.enabled {
        spawn_procedural_terrain(
            world,
            mats,
            layout.terrain,
            materials.terrain,
            &map.terrain,
            map.palette.terrain,
            initial_terrain_center,
        )
    } else {
        spawn_authored_terrain_reference(world, layout.terrain, &map.terrain)
    };
    let static_world = begin_static_world_prefabs(world, mats, layout.terrain, &map.prefabs);
    spawn_foliage_prefabs(
        world,
        prims,
        mats,
        layout.foliage,
        terrain,
        materials,
        &map.materials,
        &map.palette,
        &map.foliage,
        &map.prefabs,
        map.player.start,
    );
    spawn_skydome(
        world,
        prims,
        mats,
        materials,
        layout.environment,
        &map.sky,
        map.palette.sky,
    );
    instantiate_game_ready_definitions(world, layout.definitions, &map.definitions);

    let start_x = map.player.start.x;
    let start_z = map.player.start.z;
    let player_tuning = rules.player.sanitized();
    let start_y = terrain_height(world, terrain, start_x, start_z)
        + map.player.start.y
        + player_tuning.body_half_height
        + player_tuning.body_radius
        + player_tuning.contact_skin;
    let player = spawn_player_controller(
        world,
        Some(layout.actors),
        "Player/FPS",
        Vec3::new(start_x, start_y, start_z),
        character_body_from_fps_tuning(player_tuning),
        character_motion_from_fps_tuning(player_tuning),
        false,
    );
    newengine_engine_runtime::gameplay::attach_scene_element_core(
        world,
        player,
        newengine_engine_runtime::gameplay::SceneEntityRole::Player,
        "Scene/Actors/Player",
        Vec3::new(start_x, start_y, start_z),
        Vec3::new(
            player_tuning.body_radius,
            player_tuning.body_half_height + player_tuning.body_radius,
            player_tuning.body_radius,
        ),
    );
    let model_ground_offset_y = -(player_tuning.body_half_height + player_tuning.body_radius);
    let model_bound = spawn_game_ready_player_model(
        world,
        prims,
        mats,
        player,
        &map.player.model,
        model_ground_offset_y,
    );
    if !model_bound {
        newengine_ulog_api::ulog::warn!(
            "game-ready: player runtime model disabled or unavailable; player visual was not spawned because authored model data is required"
        );
    }
    if let Some(motor) = world.get_mut::<newengine_sim::CharacterMotor>(player) {
        motor.move_speed = map.player.move_speed;
        motor.look_sens = map.player.look_sens;
        motor.yaw = map.player.yaw;
    }
    if let Some(t) = world.get_mut_tracked::<Transform>(player) {
        t.rotation = Quat::from_euler(EulerRot::YXZ, map.player.yaw, 0.0, 0.0);
    }

    if let Some(cam) = active_camera {
        let _ = set_parent(world, cam, Some(layout.cameras));
        newengine_engine_runtime::gameplay::attach_scene_element_core(
            world,
            cam,
            newengine_engine_runtime::gameplay::SceneEntityRole::ActiveCamera,
            "Scene/Cameras/ActiveCamera",
            Vec3::new(start_x, start_y + player_tuning.camera_eye_height, start_z),
            Vec3::splat(0.35),
        );
        if let Some(t) = world.get_mut_tracked::<Transform>(cam) {
            t.position = Vec3::new(start_x, start_y + player_tuning.camera_eye_height, start_z);
            t.rotation = Quat::from_euler(EulerRot::YXZ, map.player.yaw, 0.0, 0.0);
        }
    }

    let mission = spawn_game_ready_mission(
        world,
        prims,
        mats,
        layout.actors,
        terrain,
        &map.gameplay.mission,
    );
    world.insert_resource(FpsDemoState::from_rules_with_targets(
        mission.pickups,
        mission.targets,
        map.title.clone(),
        map.objective.clone(),
        &rules,
    ));

    super::shadow_torture::bootstrap_if_requested(
        world,
        prims,
        mats,
        layout.actors,
        terrain,
        materials,
        map.player.start,
    );

    newengine_ulog_api::ulog::info!(
        "game-ready bootstrap summary: title='{}' objective='{}' player={:?} terrain={:?} camera={:?} player_model_bound={} definitions={} prefabs={} static_world_models={} static_world_parts={} static_world_triangles={} mission_pickups={} mission_targets={} mission_hazards={} mission_goals={} foliage_enabled={} terrain_streaming_enabled={} terrain_chunk_radius={} terrain_unload_radius={} sky_mesh='{}' sky_definition_ref='{}' layout_environment={:?} layout_terrain={:?} layout_foliage={:?} layout_definitions={:?} layout_actors={:?} layout_cameras={:?}",
        map.title,
        map.objective,
        player,
        terrain,
        active_camera,
        model_bound,
        map.definitions.len(),
        map.prefabs.len(),
        static_world.models,
        static_world.parts,
        static_world.triangles,
        mission.pickups,
        mission.targets,
        mission.hazards,
        mission.goals,
        map.foliage.enabled,
        map.terrain.enabled && map.terrain.streaming.enabled,
        map.terrain.streaming.chunk_radius,
        map.terrain.streaming.unload_radius,
        map.sky.mesh,
        map.sky.definition_ref,
        layout.environment,
        layout.terrain,
        layout.foliage,
        layout.definitions,
        layout.actors,
        layout.cameras
    );

    let object_invariant_report = {
        let world = scene.world_mut();
        validate_scene_objects(world, "game-ready.bootstrap")
    };
    let invariants_repaired = scene.validate_invariants();
    newengine_ulog_api::ulog::info!(
        "game-ready scene object invariants summary: checked={} repaired={} missing_transform={} missing_bounds={} missing_physics={} policy='new scene objects cannot remain incomplete'",
        object_invariant_report.checked,
        object_invariant_report.repaired,
        object_invariant_report.missing_transform,
        object_invariant_report.missing_bounds,
        object_invariant_report.missing_physics
    );
    if invariants_repaired {
        newengine_ulog_api::ulog::warn!(
            "game-ready bootstrap invariants: status='repaired' selected_player={:?} meaning='Scene::validate_invariants changed SceneState/unique markers during reconciliation'",
            player
        );
    } else {
        newengine_ulog_api::ulog::info!(
            "game-ready bootstrap invariants: status='stable' selected_player={:?} meaning='SceneState and unique markers were already consistent'",
            player
        );
    }
    Some(player)
}

#[cfg(test)]
mod game_data_lighting_tests {
    use super::*;

    #[test]
    fn project_game_data_is_authoritative_for_shadow_quality() {
        let mut data = newengine_game_data::default_game_data().clone();
        data.world.lighting.sun_intensity = 3.25;
        data.world.shadows.cascade_count = 1;
        data.world.shadows.max_distance = 96.0;
        data.world.shadows.filter = "pcf".to_owned();
        data.world.day_night.time_of_day_hours = 11.5;

        let spec = game_data_lighting_spec(&data);
        assert_eq!(spec.shadows.cascade_count, 1);
        assert_eq!(spec.shadows.max_distance, 96.0);
        assert_eq!(spec.shadows.filter, newengine_lighting::ShadowFilter::Pcf);
        assert_eq!(spec.sun_intensity, 3.25);
        assert_eq!(spec.day_night.time_of_day_hours, 11.5);
    }

    #[test]
    fn project_game_data_lighting_is_sanitized_before_runtime_install() {
        let mut data = newengine_game_data::default_game_data().clone();
        data.world.shadows.cascade_count = 99;
        data.world.shadows.max_distance = 50_000.0;
        data.world.shadows.filter = "unknown".to_owned();
        data.world.day_night.day_of_year = 999;
        data.world.day_night.latitude_degrees = 120.0;

        let spec = game_data_lighting_spec(&data);
        assert_eq!(spec.shadows.cascade_count, 4);
        assert_eq!(spec.shadows.max_distance, 1000.0);
        assert_eq!(spec.shadows.filter, newengine_lighting::ShadowFilter::Pcf);
        assert_eq!(spec.day_night.day_of_year, 366);
        assert_eq!(spec.day_night.latitude_degrees, 89.0);
    }
}
