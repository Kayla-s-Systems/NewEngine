#[inline]
fn mission_position(
    world: &newengine_ecs::World,
    terrain: EntityId,
    authored: Vec3,
    center_offset: f32,
) -> Vec3 {
    Vec3::new(
        authored.x,
        terrain_height(world, terrain, authored.x, authored.z) + authored.y + center_offset,
        authored.z,
    )
}

fn spawn_mission_primitive(
    world: &mut newengine_ecs::World,
    prims: &PrimitiveRegistry,
    mats: &MaterialRegistry,
    parent: EntityId,
    material_id: MaterialId,
    primitive_id: PrimitiveId,
    name: &str,
    position: Vec3,
    scale: Vec3,
) -> EntityId {
    spawn_game_primitive(
        world,
        prims,
        mats,
        PrimitiveSpawnSpec {
            parent,
            primitive_id,
            material_id,
            name,
            position,
            scale,
            color: [1.0, 1.0, 1.0, 1.0],
            render_options: newengine_model_domain_api::MeshRenderOptions::world_opaque(),
        },
    )
}

