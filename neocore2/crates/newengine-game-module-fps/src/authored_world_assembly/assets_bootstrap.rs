use super::*;
use newengine_authored_world_runtime::{begin_authored_map_streaming, begin_static_world_prefabs};
use newengine_engine_runtime::world_authoring::{
    spawn_primitive as spawn_game_primitive, PrimitiveSpawnSpec,
};
use newengine_fps_character_runtime::spawn_authored_player_model;
use newengine_fps_content_runtime::authored_world_profile::ytyp_metadata::{
    apply_authored_fps_ytyp_metadata, apply_required_camera_definition,
    resolve_authored_fps_asset_graph,
};
use newengine_fps_content_runtime::instantiate_authored_mission;
use newengine_game_data::{GameData, GameDataSnapshot};
use newengine_world_environment_runtime::authored_foliage::{
    defer_foliage_prefabs, spawn_foliage_prefabs, terrain_height,
};
use newengine_world_environment_runtime::authored_materials::{
    register_authored_environment_materials as register_demo_materials,
    AuthoredEnvironmentMaterials as DemoMaterials,
};
use newengine_world_environment_runtime::authored_sky::{
    attach_sky_visual_runtime, configure_authored_lighting, sky_atmosphere_from_spec,
    tick_authored_sky_cycle, SkyVisualKind, AUTHORED_SKYDOME_PRIMITIVE_ID as SKYDOME_PRIMITIVE_ID,
    SKY_VISUAL_SPAWN_ORDER,
};
use newengine_world_environment_runtime::terrain_streaming::{
    spawn_procedural_terrain, TerrainSurfaceSampler,
};

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

use audio::spawn_authored_audio_emitters;
use definitions::instantiate_authored_definitions;
use layout::{spawn_authored_scene_entity_layout, spawn_authored_terrain_reference};
use rules::to_fps_runtime_rules;
use sky_visual::spawn_skydome;

include!("assets_bootstrap/game_data.rs");

pub(crate) fn bootstrap_authored_fps_world_scene_with_resolved_map_impl(
    scene: &mut Scene,
    prims: &mut PrimitiveRegistry,
    mats: &MaterialRegistry,
    resolved_map: &newengine_authored_world_runtime::ResolvedAuthoredMapBootstrap,
) -> Option<EntityId> {
    let game_data = scene
        .world()
        .resource::<GameDataSnapshot>()
        .cloned()
        .or_else(|| {
            newengine_ulog_api::ulog::error!(
                "authored FPS world assembly requires profile-installed GameDataSnapshot before domain enrichment"
            );
            None
        })?;
    bootstrap_authored_fps_world_scene_impl_inner(scene, prims, mats, Some(resolved_map), game_data)
}

fn bootstrap_authored_fps_world_scene_impl_inner(
    scene: &mut Scene,
    prims: &mut PrimitiveRegistry,
    mats: &MaterialRegistry,
    resolved_map: Option<&newengine_authored_world_runtime::ResolvedAuthoredMapBootstrap>,
    game_data: GameDataSnapshot,
) -> Option<EntityId> {
    if resolved_map.is_none() {
        *scene = Scene::new();
        bootstrap_runtime_scene_foundation(scene);
    }

    let root = ensure_root(scene);
    let map_result = if let Some(resolved_map) = resolved_map {
        load_authored_world_profile_from_resolved_map(
            &resolved_map.logical_path,
            &resolved_map.map_ref,
            &resolved_map.index,
        )
    } else {
        load_authored_world_profile()
    };
    let mut map = match map_result {
        Ok(map) => map,
        Err(errors) => {
            newengine_ulog_api::ulog::error!(
                "fps-authored: strict data-driven scene bootstrap failed; authored .ymap was not resolved into a valid XML map profile; emergency fallback profiles are forbidden; attempts='{}'",
                errors.join(" | ")
            );
            newengine_core::crash::record_breadcrumb(format!(
                "fps-authored scene bootstrap failed: authored .ymap unresolved attempts={}",
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
            "fps-authored: project player camera bootstrap failed err='{}' policy='YMAP must declare player_camera and YTYP must provide a valid newengine.camera definition; no engine fallback'",
            error
        );
        newengine_core::crash::record_breadcrumb(format!(
            "fps-authored camera defSystem failed: {error}"
        ));
        return None;
    }
    apply_authored_fps_ytyp_metadata(&mut map, &mut effective_data);
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
        "fps-authored game-data snapshot installed source='{}' schema='{}' version={}",
        effective_game_data.source_id(),
        effective_game_data.data().schema,
        effective_game_data.data().version,
    );
    world.insert_resource(effective_game_data.clone());
    world.insert_resource(map.acoustic_materials.clone());
    newengine_ulog_api::ulog::info!(
        "fps-authored acoustic material library installed rules={} source='Shared/project YTYP metadata' fallback='transparent'",
        map.acoustic_materials.rules.len(),
    );

    let rules = to_fps_runtime_rules(&map.gameplay, effective_game_data.data());
    world.insert_resource(rules.clone());
    world.insert_resource(WorldActivationState::new(
        "waiting for CPU scene assembly and GPU material residency",
    ));
    super::runtime_contributions::install_world_runtime_adapters(world);

    let layout = spawn_authored_scene_entity_layout(world, root);

    // The project declares the player camera instance in YMAP and its behavior in YTYP.
    // FPS authored must not rely on the generic engine bootstrap manufacturing a camera.
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
            "fps-authored: instantiated authored player camera entity={:?} id='{}' definition_ref='{}' target='player'",
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
    configure_authored_lighting(
        world,
        layout.environment,
        world_instance_id,
        &map.lighting,
        &map.sky,
    );
    spawn_authored_audio_emitters(world, layout.environment, &map.audio_emitters);

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
    instantiate_authored_definitions(world, layout.definitions, &map.definitions);

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
    let model_bound = spawn_authored_player_model(
        world,
        prims,
        mats,
        player,
        &map.player.model,
        model_ground_offset_y,
    );
    if !model_bound {
        newengine_ulog_api::ulog::error!(
            "fps-authored: playable character invariant failed; character_ref='{}' model_source='{}' did not bind a visual model; invisible capsule players are forbidden",
            effective_game_data.data().player.character_ref,
            map.player.model.source,
        );
        newengine_core::crash::record_breadcrumb(format!(
            "fps-authored playable character invariant failed character_ref='{}' model_source='{}'",
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
                "fps-authored mission bootstrap rejected project content err='{}' policy='authored mission materials/content are required; no engine fallback'",
                error,
            );
            newengine_core::crash::record_breadcrumb(format!(
                "fps-authored mission bootstrap failed: {error}"
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
        newengine_fps_character_runtime::tick_player_model_assignments(world, prims, mats);

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
                    "fps-authored mission character model bound entity={} source='{}' revision={} visual_root={:?} policy='bootstrap-bound before launch; capsule physics retained; capsule render removed'",
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
                "fps-authored mission character bootstrap binding failed targets=[{}] policy='authored character targets must bind before public Play'",
                detail
            );
            newengine_core::crash::record_breadcrumb(format!(
                "fps-authored mission character bootstrap binding failed: {detail}"
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

    newengine_world_environment_runtime::shadow_validation::bootstrap_shadow_validation_if_requested(
        world,
        prims,
        mats,
        layout.actors,
        terrain,
        materials,
        map.player.start,
    );

    newengine_ulog_api::ulog::info!(
        "fps-authored bootstrap summary: title='{}' objective='{}' player={:?} terrain={:?} camera={:?} player_model_bound={} definitions={} prefabs={} static_world_models={} static_world_parts={} static_world_triangles={} mission_pickups={} inventory_item_pickups={} mission_targets={} mission_hazards={} mission_goals={} foliage_enabled={} terrain_streaming_enabled={} terrain_chunk_radius={} terrain_unload_radius={} sky_mesh='{}' sky_definition_ref='{}' layout_environment={:?} layout_terrain={:?} layout_foliage={:?} layout_definitions={:?} layout_actors={:?} layout_cameras={:?}",
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
        validate_scene_objects(world, "fps-authored.bootstrap")
    };
    let invariants_repaired = scene.validate_invariants();
    newengine_ulog_api::ulog::info!(
        "fps-authored scene object invariants summary: checked={} repaired={} missing_transform={} missing_bounds={} missing_physics={} policy='new scene objects cannot remain incomplete'",
        object_invariant_report.checked,
        object_invariant_report.repaired,
        object_invariant_report.missing_transform,
        object_invariant_report.missing_bounds,
        object_invariant_report.missing_physics
    );
    if invariants_repaired {
        newengine_ulog_api::ulog::warn!(
            "fps-authored bootstrap invariants: status='repaired' selected_player={:?} meaning='Scene::validate_invariants changed SceneState/unique markers during reconciliation'",
            player
        );
    } else {
        newengine_ulog_api::ulog::info!(
            "fps-authored bootstrap invariants: status='stable' selected_player={:?} meaning='SceneState and unique markers were already consistent'",
            player
        );
    }
    Some(player)
}

include!("assets_bootstrap/tests.rs");
