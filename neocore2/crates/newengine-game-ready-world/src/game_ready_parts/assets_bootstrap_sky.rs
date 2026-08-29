use super::*;

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
    newengine_engine_runtime::gameplay::attach_scene_element_core(
        world,
        entity,
        newengine_engine_runtime::gameplay::SceneEntityRole::SkyDome,
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
