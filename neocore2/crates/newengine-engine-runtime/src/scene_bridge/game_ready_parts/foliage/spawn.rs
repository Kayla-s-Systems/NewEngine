fn ensure_runtime_prefab_parts(
    prims: &mut PrimitiveRegistry,
    prefab: &GameReadyPrefabSpec,
    materials: DemoMaterials,
    palette: &GameReadyPaletteSpec,
) -> Result<Vec<RuntimePrefabMeshPart>, String> {
    let logical_asset = canonical_ydd_prefab_ref(prefab)?;
    let decoded = decode_runtime_ydd_prefab(&logical_asset)?;
    let mut out = Vec::with_capacity(decoded.len());
    let mut registered_parts = 0usize;
    let mut registered_vertices = 0usize;
    let mut registered_indices = 0usize;
    for part in decoded {
        let primitive_id = part.primitive_id;
        let name = part.name;
        let material_slot = part.material_slot;
        let mesh = part.mesh;
        let vertex_count = mesh.vertices.len();
        let index_count = mesh.indices.len();
        if !prims.is_registered(primitive_id) {
            prims.register_mesh(primitive_id, name.clone(), mesh);
            registered_parts += 1;
            registered_vertices += vertex_count;
            registered_indices += index_count;
            log::debug!(
                "game-ready: ydd drawable mesh registered source='{}' asset='{}' part='{}' material='{}' vertices={} indices={}",
                prefab.source,
                logical_asset,
                name,
                material_slot,
                vertex_count,
                index_count
            );
        }
        let (material_id, color) = material_for_slot(&material_slot, materials, palette);
        out.push(RuntimePrefabMeshPart {
            primitive_id,
            material_slot,
            material_id,
            color,
        });
    }
    if registered_parts > 0 {
        log::info!(
            "game-ready: ydd drawable registered source='{}' asset='{}' parts={} vertices={} indices={}",
            prefab.source,
            logical_asset,
            registered_parts,
            registered_vertices,
            registered_indices,
        );
    }
    Ok(out)
}

fn spawn_runtime_ydd_prefab_instance(
    world: &mut newengine_ecs::World,
    prims: &PrimitiveRegistry,
    mats: &MaterialRegistry,
    root: EntityId,
    parts: &[RuntimePrefabMeshPart],
    placement: TreePlacement,
) {
    let yaw = Quat::from_rotation_y(placement.yaw);
    let scale = Vec3::splat(placement.scale);
    for (part_index, part) in parts.iter().enumerate() {
        let entity = spawn_game_primitive(
            world,
            prims,
            mats,
            PrimitiveSpawnSpec {
                parent: root,
                primitive_id: part.primitive_id,
                material_id: part.material_id,
                name: &format!("Foliage/TreeAnimate-{}/{}-{part_index}", placement.index, part.material_slot),
                position: placement.position,
                scale,
                color: part.color,
            },
        );
        if let Some(t) = world.get_mut_tracked::<Transform>(entity) {
            t.rotation = yaw;
        }
    }
}

fn spawn_foliage_prefabs(
    world: &mut newengine_ecs::World,
    prims: &mut PrimitiveRegistry,
    mats: &MaterialRegistry,
    root: EntityId,
    terrain: EntityId,
    materials: DemoMaterials,
    palette: &GameReadyPaletteSpec,
    foliage: &GameReadyFoliageSpec,
    prefabs: &[GameReadyPrefabSpec],
    player_start: Vec3,
) {
    let Some(prefab) = choose_foliage_prefab(prefabs, &foliage.prefab) else {
        if foliage.enabled {
            log::warn!(
                "game-ready: foliage enabled but prefab id='{}' is not declared or disabled",
                foliage.prefab
            );
        }
        return;
    };

    let runtime_parts = match ensure_runtime_prefab_parts(prims, prefab, materials, palette) {
        Ok(parts) => parts,
        Err(e) => {
            log::error!(
                "game-ready: prefab id='{}' source='{}' proxy='{}' failed to load .ydd runtime mesh through AssetManager; foliage skipped err='{}'",
                prefab.id,
                prefab.source,
                prefab.proxy,
                e
            );
            return;
        }
    };

    let placements = collect_tree_placements(world, terrain, foliage, player_start);
    let count = placements.len();
    for placement in placements {
        spawn_runtime_ydd_prefab_instance(world, &*prims, mats, root, &runtime_parts, placement);
    }

    log_foliage_prefab_placement(
        &prefab.id,
        &prefab.source,
        &prefab.proxy,
        "ydd_runtime_mesh",
        runtime_parts.len(),
        count,
        foliage.max_count,
        foliage.grid_min,
        foliage.grid_max,
        foliage.spacing,
    );
}
