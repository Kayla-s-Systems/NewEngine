use super::foliage::{spawn_foliage_prefabs, terrain_height, SKYDOME_PRIMITIVE_ID};
use super::materials_terrain::register_demo_materials;
use super::mission::spawn_game_ready_mission;
use super::player_model::spawn_game_ready_player_model;
use super::sky::configure_game_ready_lighting;
use super::terrain_streaming::spawn_procedural_terrain;
use super::world_model::begin_static_world_prefabs;
use super::ytyp_metadata::{apply_game_ready_ytyp_metadata, resolve_game_ready_asset_graph};
use super::*;

use self::mesh_assets::ensure_skydome_primitive;

mod mesh_assets;

pub(super) fn spawn_sky_visual(
    world: &mut newengine_ecs::World,
    prims: &PrimitiveRegistry,
    mats: &MaterialRegistry,
    root: EntityId,
    material_id: MaterialId,
    primitive_id: PrimitiveId,
    spec: &GameReadySkySpec,
    kind: SkyVisualKind,
    dome_color: [f32; 4],
) -> EntityId {
    let color = kind.initial_color(dome_color);
    let entity = spawn_game_primitive(
        world,
        prims,
        mats,
        PrimitiveSpawnSpec {
            parent: root,
            primitive_id,
            material_id,
            name: kind.entity_name(),
            position: Vec3::ZERO,
            scale: Vec3::splat(kind.initial_radius(spec).max(0.1)),
            color,
            render_options: spec.render_options.clone(),
        },
    );
    attach_sky_visual_runtime(
        world,
        mats,
        entity,
        material_id,
        kind,
        color,
        (!spec.definition_ref.trim().is_empty()).then(|| spec.definition_ref.clone()),
        (!spec.mesh.trim().is_empty()).then(|| spec.mesh.clone()),
        spec.render_options.clone(),
    );
    crate::gameplay::attach_scene_element_core(
        world,
        entity,
        crate::gameplay::SceneEntityRole::SkyDome,
        "Scene/Environment/SkyDome",
        Vec3::ZERO,
        Vec3::splat(kind.initial_radius(spec).max(0.1)),
    );
    entity
}

pub(super) fn spawn_skydome(
    world: &mut newengine_ecs::World,
    prims: &mut PrimitiveRegistry,
    mats: &MaterialRegistry,
    materials: DemoMaterials,
    root: EntityId,
    spec: &GameReadySkySpec,
    color: [f32; 4],
) {
    let Some(primitive_id) = ensure_skydome_primitive(prims, &spec.mesh) else {
        world.insert_resource(sky_atmosphere_from_spec(spec));
        tick_game_ready_sky_cycle(world, 0.0);
        return;
    };

    world.insert_resource(sky_atmosphere_from_spec(spec));

    for kind in SKY_VISUAL_SPAWN_ORDER {
        let material_id = materials.sky_visual_material(kind);
        let _ = spawn_sky_visual(
            world,
            &*prims,
            mats,
            root,
            material_id,
            kind.primitive_id(primitive_id),
            spec,
            kind,
            color,
        );
    }

    tick_game_ready_sky_cycle(world, 0.0);

    newengine_ulog_api::ulog::info!(
        "game-ready skydome: follow_camera={} radius={:.1} mesh='{}' clouds='{}' profile='{}' celestial_visuals='procedural_in_sky_shader'",
        spec.follow_camera,
        spec.radius,
        spec.mesh,
        spec.cloud_dictionary,
        spec.cloud_profile,
    );
}

pub(super) fn to_fps_demo_rules(
    spec: &GameReadyGameplaySpec,
    model: &self::content::GameReadyPlayerModelSpec,
) -> FpsDemoRules {
    let default_player = FpsPlayerTuning::default();
    let base = FpsPlayerTuning {
        body_radius: spec.player_collision.radius,
        body_half_height: spec.player_collision.half_height,
        crouched_body_half_height: default_player.crouched_body_half_height,
        visual_radius: spec.player_visual.radius,
        visual_half_height: spec.player_visual.half_height,
        camera_eye_height: spec.player_visual.camera_eye_height,
        crouched_camera_eye_height: default_player.crouched_camera_eye_height,
        crouch_camera_speed: default_player.crouch_camera_speed,
        sprint_multiplier: spec.player_visual.sprint_multiplier,
        jump_speed: default_player.jump_speed,
        gravity: spec.physics.gravity,
        contact_skin: spec.physics.contact_skin,
        ground_probe_distance: default_player.ground_probe_distance,
        max_slope_radians: default_player.max_slope_radians,
        footstep_stride: default_player.footstep_stride,
        landing_speed_threshold: default_player.landing_speed_threshold,
    }
    .sanitized();
    let feet_to_eye = model.target_height * model.eye_height_ratio;
    let model_eye_offset_from_player_origin =
        feet_to_eye - (base.body_half_height + base.body_radius);
    let player = FpsPlayerTuning {
        camera_eye_height: model_eye_offset_from_player_origin.clamp(0.05, model.target_height),
        ..base
    }
    .sanitized();

    FpsDemoRules {
        default_status: spec.default_status.clone(),
        pickup_status: spec.pickup_status.clone(),
        target_status: spec.target_status.clone(),
        hazard_status: spec.hazard_status.clone(),
        goal_locked_status: spec.goal_locked_status.clone(),
        goal_complete_status: spec.goal_complete_status.clone(),
        failed_progress_label: spec.failed_progress_label.clone(),
        completed_progress_label: spec.completed_progress_label.clone(),
        player,
    }
}

#[derive(Clone, Copy, Debug)]
struct GameReadySceneEntityLayout {
    environment: EntityId,
    terrain: EntityId,
    foliage: EntityId,
    definitions: EntityId,
    actors: EntityId,
    cameras: EntityId,
}

fn spawn_scene_layout_node(
    world: &mut newengine_ecs::World,
    parent: EntityId,
    name: &'static str,
    role: crate::gameplay::SceneEntityRole,
) -> EntityId {
    let entity = spawn_named(world, name);
    let _ = set_parent(world, entity, Some(parent));
    crate::gameplay::attach_scene_element_core(
        world,
        entity,
        role,
        name,
        Vec3::ZERO,
        Vec3::splat(0.25),
    );
    entity
}

fn spawn_game_ready_scene_entity_layout(
    world: &mut newengine_ecs::World,
    root: EntityId,
) -> GameReadySceneEntityLayout {
    let layout = GameReadySceneEntityLayout {
        environment: spawn_scene_layout_node(
            world,
            root,
            "Scene/Environment",
            crate::gameplay::SceneEntityRole::Environment,
        ),
        terrain: spawn_scene_layout_node(
            world,
            root,
            "Scene/Terrain",
            crate::gameplay::SceneEntityRole::Terrain,
        ),
        foliage: spawn_scene_layout_node(
            world,
            root,
            "Scene/Foliage",
            crate::gameplay::SceneEntityRole::Foliage,
        ),
        definitions: spawn_scene_layout_node(
            world,
            root,
            "Scene/Definitions",
            crate::gameplay::SceneEntityRole::Definitions,
        ),
        actors: spawn_scene_layout_node(
            world,
            root,
            "Scene/Actors",
            crate::gameplay::SceneEntityRole::Actors,
        ),
        cameras: spawn_scene_layout_node(
            world,
            root,
            "Scene/Cameras",
            crate::gameplay::SceneEntityRole::Cameras,
        ),
    };
    newengine_ulog_api::ulog::info!(
        "game-ready scene layout: all authored scene elements are ordinary ECS entities environment={:?} terrain={:?} foliage={:?} definitions={:?} actors={:?} cameras={:?} policy='no special scene side-channel elements'",
        layout.environment,
        layout.terrain,
        layout.foliage,
        layout.definitions,
        layout.actors,
        layout.cameras
    );
    layout
}

fn spawn_authored_terrain_reference(
    world: &mut newengine_ecs::World,
    parent: EntityId,
    spec: &GameReadyTerrainSpec,
) -> EntityId {
    let entity = spawn_named(world, "Scene/Terrain/AuthoredWorldReference");
    let _ = set_parent(world, entity, Some(parent));
    let _ = world.insert(
        entity,
        Transform {
            position: Vec3::new(0.0, spec.base_height, 0.0),
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        },
    );
    newengine_ulog_api::ulog::info!(
        "game-ready terrain: procedural terrain disabled; authored world reference entity={:?} base_height={} policy='no default terrain mesh, no default terrain collider'",
        entity,
        spec.base_height
    );
    entity
}

pub(super) fn instantiate_game_ready_definitions(
    world: &mut newengine_ecs::World,
    root: EntityId,
    definitions: &[GameReadyDefinitionInstanceSpec],
) {
    if definitions.is_empty() {
        return;
    }
    newengine_ulog_api::ulog::debug!(
        "definitions.runtime: game-ready definition batch count={} policy='.ymap placements declare apply_mode; .ytyp dependencies are graph inputs, not implicit render/spawn commands'",
        definitions.len()
    );
    for spec in definitions {
        let graph = resolve_game_ready_asset_graph(&spec.definition_ref).unwrap_or_else(|| {
            newengine_model_domain_api::AssetGraphResolver::resolve_root_ref(&spec.definition_ref)
        });
        if matches!(spec.apply_mode, GameReadyDefinitionApplyMode::MetadataOnly) {
            newengine_ulog_api::ulog::debug!(
                "definitions.runtime: metadata-only definition_ref='{}' nodes={} missing={} apply_mode='{}' policy='domain systems consume engine.assets.definitions/engine.assets.graph explicitly; no generic ECS/render marker spawned'",
                spec.definition_ref,
                graph.nodes.len(),
                graph.missing_refs.len(),
                spec.apply_mode.as_str()
            );
            continue;
        }

        let transform = crate::scene_bridge::definitions_runtime::DefinitionInstantiateTransform {
            translation: [spec.position.x, spec.position.y, spec.position.z],
            rotation_ypr: spec.rotation_ypr,
            scale: [spec.scale.x, spec.scale.y, spec.scale.z],
        };
        let (entity, trace) =
            crate::scene_bridge::definitions_runtime::apply_definition_instantiation(
                world,
                Some(root),
                spec.definition_ref.clone(),
                transform,
                graph,
            );
        newengine_ulog_api::ulog::debug!(
            "definitions.runtime: instantiated marker definition_ref='{}' entity={:?} nodes={} missing={} render_drawables={} materials={} textures={} physics_refs={} result='{}' apply_mode='{}'",
            trace.definition_ref,
            entity,
            trace.resolved_graph.nodes.len(),
            trace.resolved_graph.missing_refs.len(),
            trace.render_packet_request.drawable_refs.len(),
            trace.render_packet_request.material_refs.len(),
            trace.render_packet_request.texture_refs.len(),
            trace.physics_declaration.collision_refs.len() + trace.physics_declaration.physics_refs.len(),
            trace.apply_result,
            spec.apply_mode.as_str()
        );
    }
}

pub(in crate::scene_bridge) fn bootstrap_fps_game_ready_scene(
    scene: &mut Scene,
    prims: &mut PrimitiveRegistry,
    mats: &MaterialRegistry,
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
    let materials = register_demo_materials(mats, &map.palette, &map.materials);
    let world = scene.world_mut();

    let rules = to_fps_demo_rules(&map.gameplay, &map.player.model);
    world.insert_resource(rules.clone());
    world.insert_resource(GameReadyWorldLaunchGate::new(
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
    let player = spawn_player_controller_with_tuning(
        world,
        Some(layout.actors),
        "Player/FPS",
        Vec3::new(start_x, start_y, start_z),
        player_tuning,
        false,
    );
    crate::gameplay::attach_scene_element_core(
        world,
        player,
        crate::gameplay::SceneEntityRole::Player,
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
        crate::gameplay::attach_scene_element_core(
            world,
            cam,
            crate::gameplay::SceneEntityRole::ActiveCamera,
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
        validate_scene_object_invariants(world, "game-ready.bootstrap")
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
