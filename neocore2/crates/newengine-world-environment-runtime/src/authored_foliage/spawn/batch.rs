fn spawn_runtime_ydd_prefab_batch(
    world: &mut newengine_ecs::World,
    prims: &mut PrimitiveRegistry,
    mats: &MaterialRegistry,
    root: EntityId,
    prefab: &AuthoredWorldPlacementSpec,
    parts: &[RuntimePrefabMeshPart],
    placements: &[TreePlacement],
    render_options: &newengine_model_domain_api::MeshRenderOptions,
) -> Result<usize, String> {
    let mut spawned = 0usize;
    for (part_index, part) in parts.iter().enumerate() {
        let mesh = bake_foliage_mesh(prims, part, placements)?;
        let vertices = mesh.vertices.len();
        let indices = mesh.indices.len();
        let primitive_id = PrimitiveId(fnv1a_64(&format!(
            "authored-environment.foliage.static-batch:{}:{}:{}",
            prefab.source, part.material_slot, part_index
        )));
        // register_mesh intentionally replaces an older batch. Scene reloads may
        // change seed/density while the registry outlives the ECS world.
        prims.register_mesh(
            primitive_id,
            format!("Foliage/StaticBatch/{}", part.material_slot),
            mesh,
        );
        let _entity = spawn_game_primitive(
            world,
            &*prims,
            mats,
            PrimitiveSpawnSpec {
                parent: root,
                primitive_id,
                material_id: part.material_id,
                name: &format!("Foliage/StaticBatch/{}-{part_index}", part.material_slot),
                position: Vec3::ZERO,
                scale: Vec3::ONE,
                color: part.color,
                render_options: render_options.clone(),
            },
        );
        spawned = spawned.saturating_add(1);
        newengine_ulog_api::ulog::debug!(
            "authored-environment foliage batch part: prefab='{}' slot='{}' placements={} vertices={} triangles={} primitive={:?}",
            prefab.id,
            part.material_slot,
            placements.len(),
            vertices,
            indices / 3,
            primitive_id,
        );
    }
    Ok(spawned)
}
