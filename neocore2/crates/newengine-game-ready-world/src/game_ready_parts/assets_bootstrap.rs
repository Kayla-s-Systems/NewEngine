use super::foliage::{
    defer_foliage_prefabs, spawn_foliage_prefabs, terrain_height, SKYDOME_PRIMITIVE_ID,
};
use super::materials_terrain::register_demo_materials;
use super::mission::instantiate_authored_mission;
use super::player_model::spawn_game_ready_player_model;
use super::sky::configure_game_ready_lighting;
use super::terrain_streaming::spawn_procedural_terrain;
use super::world_model::{begin_authored_map_streaming, begin_static_world_prefabs};
use super::ytyp_metadata::{
    apply_game_ready_ytyp_metadata, apply_required_camera_definition,
    resolve_game_ready_asset_graph,
};
use super::*;
use newengine_game_data::{GameData, GameDataSnapshot};

use self::mesh_assets::ensure_skydome_primitive;

mod mesh_assets;

#[path = "assets_bootstrap_audio.rs"]
mod audio;
#[path = "assets_bootstrap_definitions.rs"]
mod definitions;
#[path = "assets_bootstrap_layout.rs"]
mod layout;
#[path = "assets_bootstrap_rules.rs"]
mod rules;
#[path = "assets_bootstrap_sky.rs"]
mod sky_visual;

use audio::spawn_game_ready_audio_emitters;
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
fn game_data_sky_spec(data: &GameData, fallback: &GameReadySkySpec) -> GameReadySkySpec {
    let sky = &data.world.sky;
    let definition_ref = sky.definition_ref.trim().replace('\\', "/");
    let mesh = sky.mesh.trim().replace('\\', "/");
    let cloud_dictionary = sky.cloud_dictionary.trim().replace('\\', "/");
    let moon_texture = sky.moon_texture.trim().replace('\\', "/");
    GameReadySkySpec {
        definition_ref: if definition_ref.is_empty() {
            fallback.definition_ref.clone()
        } else {
            definition_ref
        },
        render_options: fallback.render_options.clone(),
        radius: sky.radius.max(0.1),
        // Asset identity belongs to the selected YTYP graph. Empty GameData fields
        // mean "use the selected sky definition", not "erase the resolved asset".
        mesh: if mesh.is_empty() {
            fallback.mesh.clone()
        } else {
            mesh
        },
        follow_camera: sky.follow_camera,
        environment_profile: sky.environment_profile.trim().to_owned(),
        environment_region: sky.environment_region.trim().to_owned(),
        environment_biome: sky.environment_biome.trim().to_owned(),
        cloud_dictionary: if cloud_dictionary.is_empty() {
            fallback.cloud_dictionary.clone()
        } else {
            cloud_dictionary
        },
        cloud_profile: sky.cloud_profile.trim().to_owned(),
        sun_radius: sky.sun_radius.max(0.1),
        moon_radius: sky.moon_radius.max(0.1),
        moon_texture: if moon_texture.is_empty() {
            fallback.moon_texture.clone()
        } else {
            moon_texture
        },
        atmosphere: GameReadySkyAtmosphereSpec {
            day_zenith: sky.atmosphere.day_zenith,
            day_horizon: sky.atmosphere.day_horizon,
            dusk_zenith: sky.atmosphere.dusk_zenith,
            dusk_horizon: sky.atmosphere.dusk_horizon,
            night_zenith: sky.atmosphere.night_zenith,
            night_horizon: sky.atmosphere.night_horizon,
            cloud_day: sky.atmosphere.cloud_day,
            cloud_night: sky.atmosphere.cloud_night,
            night_sky_strength: sky.atmosphere.night_sky_strength.max(0.0),
            cloud_coverage: sky.atmosphere.cloud_coverage.clamp(0.0, 1.0),
            cloud_softness: sky.atmosphere.cloud_softness.clamp(0.0, 1.0),
        },
    }
}

fn install_game_data_sky_definition(map: &mut AuthoredWorldProfile, data: &GameData) {
    let previous_sky_ref = map.sky.definition_ref.trim().replace('\\', "/");
    map.sky = game_data_sky_spec(data, &map.sky);
    let definition_ref = map.sky.definition_ref.trim();
    if definition_ref.is_empty() {
        return;
    }
    if !previous_sky_ref.is_empty() && previous_sky_ref != definition_ref {
        map.definitions.retain(|spec| {
            spec.apply_mode != GameReadyDefinitionApplyMode::MetadataOnly
                || spec.definition_ref != previous_sky_ref
        });
    }
    if !map
        .definitions
        .iter()
        .any(|spec| spec.definition_ref == definition_ref)
    {
        map.definitions.push(GameReadyDefinitionInstanceSpec {
            definition_ref: definition_ref.to_owned(),
            position: Vec3::ZERO,
            rotation_ypr: [0.0, 0.0, 0.0],
            scale: Vec3::ONE,
            apply_mode: GameReadyDefinitionApplyMode::MetadataOnly,
        });
    }
}

fn install_game_data_player_definition(map: &mut AuthoredWorldProfile, data: &GameData) {
    let map_owns_avatar = map.player.model.enabled && !map.player.model.source.trim().is_empty();
    if map_owns_avatar {
        return;
    }

    let definition_ref = data.player.character_ref.trim().replace('\\', "/");
    if definition_ref.is_empty() {
        return;
    }
    if !map
        .definitions
        .iter()
        .any(|spec| spec.definition_ref == definition_ref)
    {
        map.definitions.push(GameReadyDefinitionInstanceSpec {
            definition_ref: definition_ref.clone(),
            position: Vec3::ZERO,
            rotation_ypr: [0.0, 0.0, 0.0],
            scale: Vec3::ONE,
            apply_mode: GameReadyDefinitionApplyMode::MetadataOnly,
        });
    }
    newengine_ulog_api::ulog::info!(
        "game-ready player character selection: definition_ref='{}' policy='every game has a playable visual character; Shared preset hydrates model and character tuning'",
        definition_ref
    );
}

fn install_game_data_player_input_policy(profile: &mut AuthoredWorldProfile, data: &GameData) {
    // GameData V2 owns only project-level player input policy here. Character model, body,
    // movement speeds and locomotion tuning are definition-owned and are hydrated from the
    // selected character YTYP below. Never clamp the V2 runtime-resolved sentinel fields into
    // synthetic character defaults.
    profile.player.look_sens = data.player.look_sensitivity;
    newengine_ulog_api::ulog::info!(
        "game-ready game-data player input policy: look_sensitivity={:.6} character_ref='{}' policy='GameData selects character/input policy; YMAP owns spawn; YTYP owns model/body/locomotion'",
        profile.player.look_sens,
        data.player.character_ref,
    );
}

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
    bootstrap_runtime_scene_foundation(scene);

    let root = ensure_root(scene);
    let mut map = match load_authored_world_profile() {
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
    // Project GameData selects player/input policy. YMAP owns topology/spawn and the selected
    // character YTYP owns the complete model/body/locomotion contract.
    install_game_data_player_input_policy(&mut map, game_data.data());
    install_game_data_sky_definition(&mut map, game_data.data());
    install_game_data_player_definition(&mut map, game_data.data());

    let mut effective_data = game_data.data().clone();
    if let Err(error) = apply_required_camera_definition(&mut map) {
        newengine_ulog_api::ulog::error!(
            "game-ready: project player camera bootstrap failed err='{}' policy='YMAP must declare player_camera and YTYP must provide a valid newengine.camera definition; no engine fallback'",
            error
        );
        newengine_core::crash::record_breadcrumb(format!(
            "game-ready camera defSystem failed: {error}"
        ));
        return None;
    }
    apply_game_ready_ytyp_metadata(&mut map, &mut effective_data);
    // GameData is authoritative for runtime weather/atmosphere policy, while the
    // selected YTYP owns the skydome/material/texture graph. The merge below keeps
    // resolved YTYP assets whenever GameData intentionally leaves asset fields empty.
    map.sky = game_data_sky_spec(&effective_data, &map.sky);
    let effective_game_data = GameDataSnapshot::new(
        format!(
            "{}+character:{}",
            game_data.source_id(),
            effective_data.player.character_ref
        ),
        effective_data,
    );
    // Apply project/provider lighting before any light or sky entity is constructed.
    map.lighting = game_data_lighting_spec(effective_game_data.data());
    let materials = register_demo_materials(mats, &map.palette, &map.materials);
    let world = scene.world_mut();
    newengine_ulog_api::ulog::info!(
        "game-ready game-data snapshot installed source='{}' schema='{}' version={}",
        effective_game_data.source_id(),
        effective_game_data.data().schema,
        effective_game_data.data().version,
    );
    world.insert_resource(effective_game_data.clone());
    world.insert_resource(map.acoustic_materials.clone());
    newengine_ulog_api::ulog::info!(
        "game-ready acoustic material library installed rules={} source='Shared/project YTYP metadata' fallback='transparent'",
        map.acoustic_materials.rules.len(),
    );

    let rules = to_fps_demo_rules(&map.gameplay, effective_game_data.data());
    world.insert_resource(rules.clone());
    world.insert_resource(WorldActivationState::new(
        "waiting for CPU scene assembly and GPU material residency",
    ));

    let layout = spawn_game_ready_scene_entity_layout(world, root);

    // The project declares the player camera instance in YMAP and its behavior in YTYP.
    // GameReady must not rely on the generic engine bootstrap manufacturing a camera.
    let active_camera = {
        let camera = spawn_named(world, "PlayerCamera");
        let _ = world.insert(camera, newengine_scene::components::ActiveCamera);
        let _ = world.insert(
            camera,
            newengine_camera::CameraDefinitionBinding::player(
                map.gameplay.camera.instance_id.clone(),
                map.gameplay.camera.definition_ref.clone(),
            ),
        );
        let _ = world.insert(camera, newengine_sim::CameraRigComp::default());
        let _ = world.insert(camera, newengine_camera::RuntimeNavController::default());
        let _ = set_parent(world, camera, Some(layout.cameras));
        newengine_engine_runtime::gameplay::attach_scene_element_core(
            world,
            camera,
            newengine_engine_runtime::gameplay::SceneEntityRole::ActiveCamera,
            "Scene/Cameras/PlayerCamera",
            map.gameplay.camera.position,
            Vec3::splat(0.35),
        );
        if let Some(transform) = world.get_mut_tracked::<Transform>(camera) {
            transform.position = map.gameplay.camera.position;
            transform.rotation = Quat::from_euler(
                EulerRot::YXZ,
                map.gameplay.camera.rotation_ypr.x,
                map.gameplay.camera.rotation_ypr.y,
                map.gameplay.camera.rotation_ypr.z,
            );
        }
        if let Some(state) = world.resource_mut::<newengine_scene::SceneState>() {
            state.active_camera = Some(camera);
        } else {
            world.insert_resource(newengine_scene::SceneState::new(Some(root), Some(camera)));
        }
        newengine_ulog_api::ulog::info!(
            "game-ready: instantiated authored player camera entity={:?} id='{}' definition_ref='{}' target='player'",
            camera,
            map.gameplay.camera.instance_id,
            map.gameplay.camera.definition_ref,
        );
        camera
    };

    let world_instance_id = map
        .authored_map_streaming
        .as_ref()
        .map(|streaming| streaming.map_ref.trim())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| map.title.trim());
    if world_instance_id.is_empty() {
        newengine_ulog_api::ulog::error!(
            "world bootstrap rejected project content: authored world identity is empty; provide a map_ref or non-empty world title"
        );
        return None;
    }
    configure_game_ready_lighting(
        world,
        layout.environment,
        world_instance_id,
        &map.lighting,
        &map.sky,
    );
    spawn_game_ready_audio_emitters(world, layout.environment, &map.audio_emitters);

    let initial_terrain_center = newengine_scene::SceneCellCoord::from_world_pos(
        map.player.start,
        map.terrain.size_x,
        map.terrain.size_z,
    );
    let (terrain, terrain_surface) = if map.terrain.enabled {
        let (entity, sampler) = spawn_procedural_terrain(
            world,
            mats,
            layout.terrain,
            materials.terrain,
            &map.terrain,
            map.palette.terrain,
            initial_terrain_center,
        );
        (entity, Some(sampler))
    } else {
        let entity = spawn_authored_terrain_reference(world, layout.terrain, &map.terrain);
        let sampler = TerrainSurfaceSampler::flat(
            Vec3::new(0.0, map.terrain.base_height, 0.0),
            map.terrain.size_x,
            map.terrain.size_z,
        );
        (entity, Some(sampler))
    };
    begin_authored_map_streaming(world, layout.terrain, map.authored_map_streaming.as_ref());
    let static_world = begin_static_world_prefabs(world, mats, layout.terrain, &map.prefabs);
    if map.terrain.enabled {
        spawn_foliage_prefabs(
            world,
            prims,
            mats,
            layout.foliage,
            terrain,
            terrain_surface.as_ref(),
            materials,
            &map.materials,
            &map.palette,
            &map.foliage,
            &map.prefabs,
            map.player.start,
        );
    } else {
        defer_foliage_prefabs(
            world,
            layout.foliage,
            terrain,
            terrain_surface.clone(),
            materials,
            &map.materials,
            &map.palette,
            &map.foliage,
            &map.prefabs,
            map.player.start,
        );
    }
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
    let start_y = terrain_surface
        .as_ref()
        .map(|surface| surface.sample_world_height(start_x, start_z))
        .unwrap_or_else(|| terrain_height(world, terrain, start_x, start_z))
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
    // Camera behavior is project/scene-authored. Install the resolved data contract before
    // binding the character so first-person owner visibility is known during mesh creation.
    let player_camera_profile = map.gameplay.camera.player_profile();
    let _ = world.insert(player, player_camera_profile);
    if let Some(barrier) = world
        .get_mut::<newengine_engine_runtime::gameplay::PlayerFirstPersonBodyBarrierProfile>(
        player,
    ) {
        barrier.downward_pitch_limit_radians =
            player_camera_profile.first_person_down_pitch_limit_radians;
    }

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
        newengine_ulog_api::ulog::error!(
            "game-ready: playable character invariant failed; character_ref='{}' model_source='{}' did not bind a visual model; invisible capsule players are forbidden",
            effective_game_data.data().player.character_ref,
            map.player.model.source,
        );
        newengine_core::crash::record_breadcrumb(format!(
            "game-ready playable character invariant failed character_ref='{}' model_source='{}'",
            effective_game_data.data().player.character_ref,
            map.player.model.source,
        ));
        return None;
    }
    let movement_speeds = newengine_engine_runtime::gameplay::PlayerMovementSpeeds {
        walk: map.player.walk_speed,
        run: map.player.run_speed,
        sprint: map.player.sprint_speed,
        crouch: map.player.crouch_speed,
    }
    .sanitized();
    let _ = world.insert(player, movement_speeds);
    let _ = world.insert(
        player,
        newengine_engine_runtime::gameplay::Health::new(map.player.health_maximum),
    );
    if let Some(combat_team) = map.player.combat_team {
        let _ = world.insert(
            player,
            newengine_engine_runtime::gameplay::CombatTeam::new(combat_team),
        );
    }
    let _ = world.insert(
        player,
        newengine_engine_runtime::gameplay::Stamina::new(map.player.stamina_maximum),
    );
    let _ = world.insert(
        player,
        newengine_engine_runtime::gameplay::StaminaTuning {
            sprint_drain_per_second: map.player.stamina_sprint_drain_per_second,
            regen_per_second: map.player.stamina_regen_per_second,
            regen_delay_seconds: map.player.stamina_regen_delay_seconds,
            exhausted_resume_fraction: map.player.stamina_exhausted_resume_fraction,
        }
        .sanitized(),
    );
    let _ = world.insert(
        player,
        newengine_engine_runtime::gameplay::DamageReceiver::character(),
    );
    let _ = world.insert(player, map.player.damage_response_tuning);
    let _ = world.insert(player, map.player.death_policy);
    let _ = world.insert(
        player,
        newengine_engine_runtime::gameplay::CharacterInjuryState::default(),
    );
    if let Some(motion) =
        world.get_mut::<newengine_engine_runtime::gameplay::CharacterMotionTuning>(player)
    {
        // Keep legacy consumers (camera/debug bridges) coherent with the authored absolute speeds.
        motion.sprint_multiplier = movement_speeds.sprint_multiplier();
    }
    if let Some(motor) = world.get_mut::<newengine_sim::CharacterMotor>(player) {
        motor.move_speed = movement_speeds.run;
        motor.look_sens = map.player.look_sens;
        motor.yaw = map.player.yaw;
    }
    if let Some(t) = world.get_mut_tracked::<Transform>(player) {
        t.rotation = Quat::from_euler(EulerRot::YXZ, map.player.yaw, 0.0, 0.0);
    }

    // Possession will take over camera motion, but seed the declared camera from the resolved
    // player start so the first pre-possession frame is finite and deterministic.
    if let Some(t) = world.get_mut_tracked::<Transform>(active_camera) {
        t.position = Vec3::new(start_x, start_y + player_tuning.camera_eye_height, start_z);
        t.rotation = Quat::from_euler(EulerRot::YXZ, map.player.yaw, 0.0, 0.0);
    }

    let mission = match instantiate_authored_mission(
        world,
        prims,
        mats,
        layout.actors,
        terrain,
        &map.gameplay.mission,
    ) {
        Ok(mission) => mission,
        Err(error) => {
            newengine_ulog_api::ulog::error!(
                "game-ready mission bootstrap rejected project content err='{}' policy='authored mission materials/content are required; no engine fallback'",
                error,
            );
            newengine_core::crash::record_breadcrumb(format!(
                "game-ready mission bootstrap failed: {error}"
            ));
            return None;
        }
    };
    // Mission character assignments are authored during mission spawn, after the playable avatar
    // has already passed its bootstrap bind. Admit those NPC character models immediately so the
    // public launch gate can never expose the diagnostic capsule for a character-backed target.
    let mission_character_assignments = world
        .query::<newengine_engine_runtime::gameplay::PlayerModelAssignment>()
        .filter_map(|(entity, assignment)| {
            (world.get::<FpsObjectiveTarget>(entity).is_some()
                && assignment.enabled
                && !assignment.source.trim().is_empty())
            .then_some((entity, assignment.clone()))
        })
        .collect::<Vec<_>>();
    if !mission_character_assignments.is_empty() {
        player_model::tick_player_model_assignments(world, prims, mats);

        let mut failed = Vec::new();
        for (entity, assignment) in mission_character_assignments {
            let bound = world
                .get::<newengine_engine_runtime::gameplay::PlayerModelBinding>(entity)
                .filter(|binding| {
                    binding.assignment_revision == assignment.revision
                        && binding.source == assignment.source
                        && binding.visual_root.is_some_and(|root| world.exists(root))
                });
            if let Some(binding) = bound {
                let bound_source = binding.source.clone();
                let bound_revision = binding.assignment_revision;
                let bound_visual_root = binding.visual_root;
                // The capsule remains authoritative for physics only. Once the authored character
                // visual is live, remove its diagnostic render primitive so two bodies cannot overlap.
                let _ = world.remove::<Primitive>(entity);
                newengine_ulog_api::ulog::info!(
                    "game-ready mission character model bound entity={} source='{}' revision={} visual_root={:?} policy='bootstrap-bound before launch; capsule physics retained; capsule render removed'",
                    entity.stable_u64(),
                    bound_source,
                    bound_revision,
                    bound_visual_root,
                );
            } else {
                failed.push(format!(
                    "entity={} source='{}' revision={}",
                    entity.stable_u64(),
                    assignment.source,
                    assignment.revision
                ));
            }
        }
        if !failed.is_empty() {
            let detail = failed.join(", ");
            newengine_ulog_api::ulog::error!(
                "game-ready mission character bootstrap binding failed targets=[{}] policy='authored character targets must bind before public Play'",
                detail
            );
            newengine_core::crash::record_breadcrumb(format!(
                "game-ready mission character bootstrap binding failed: {detail}"
            ));
            return None;
        }
    }

    world.insert_resource(FpsObjectiveState::from_rules_with_targets(
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
        "game-ready bootstrap summary: title='{}' objective='{}' player={:?} terrain={:?} camera={:?} player_model_bound={} definitions={} prefabs={} static_world_models={} static_world_parts={} static_world_triangles={} mission_pickups={} inventory_item_pickups={} mission_targets={} mission_hazards={} mission_goals={} foliage_enabled={} terrain_streaming_enabled={} terrain_chunk_radius={} terrain_unload_radius={} sky_mesh='{}' sky_definition_ref='{}' layout_environment={:?} layout_terrain={:?} layout_foliage={:?} layout_definitions={:?} layout_actors={:?} layout_cameras={:?}",
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
        mission.item_pickups,
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
    fn empty_game_data_sky_asset_fields_preserve_ytyp_resolved_assets() {
        let mut data = newengine_game_data::default_game_data().clone();
        data.world.sky.definition_ref =
            "shared/definitions/environment/default_sky.ytyp@default_sky".to_owned();
        data.world.sky.mesh.clear();
        data.world.sky.cloud_dictionary.clear();
        data.world.sky.moon_texture.clear();
        data.world.sky.cloud_profile = "cloudless".to_owned();

        let fallback = GameReadySkySpec {
            definition_ref: data.world.sky.definition_ref.clone(),
            render_options: newengine_model_domain_api::MeshRenderOptions::sky_background(),
            radius: 220.0,
            mesh: "models/environment/skydome.ydd@skydome_high".to_owned(),
            follow_camera: true,
            environment_profile: "environment.default".to_owned(),
            environment_region: String::new(),
            environment_biome: String::new(),
            cloud_dictionary: "textures/environment/sky_clouds_v2.ytd".to_owned(),
            cloud_profile: "temperate_cumulus_dynamic".to_owned(),
            sun_radius: 18.0,
            moon_radius: 13.5,
            moon_texture: "textures/environment/skydome.ytd@moon_new".to_owned(),
            atmosphere: GameReadySkyAtmosphereSpec {
                day_zenith: [0.30, 0.55, 0.96],
                day_horizon: [0.72, 0.86, 1.0],
                dusk_zenith: [0.16, 0.20, 0.40],
                dusk_horizon: [1.0, 0.47, 0.20],
                night_zenith: [0.006, 0.010, 0.030],
                night_horizon: [0.020, 0.024, 0.052],
                cloud_day: [0.96, 0.98, 1.0],
                cloud_night: [0.04, 0.05, 0.085],
                night_sky_strength: 0.35,
                cloud_coverage: 0.0,
                cloud_softness: 0.56,
            },
        };

        let merged = game_data_sky_spec(&data, &fallback);
        assert_eq!(merged.mesh, fallback.mesh);
        assert_eq!(merged.cloud_dictionary, fallback.cloud_dictionary);
        assert_eq!(merged.moon_texture, fallback.moon_texture);
        assert_eq!(merged.cloud_profile, "cloudless");
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
