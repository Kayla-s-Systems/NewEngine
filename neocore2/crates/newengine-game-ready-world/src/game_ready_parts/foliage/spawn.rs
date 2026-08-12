use super::*;

pub(super) fn ensure_runtime_prefab_parts(
    prims: &mut PrimitiveRegistry,
    prefab: &GameReadyPrefabSpec,
    materials: DemoMaterials,
    material_specs: &GameReadyMaterialSetSpec,
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
        let material_ref = part.material_ref;
        let mesh = part.mesh;
        let vertex_count = mesh.vertices.len();
        let index_count = mesh.indices.len();
        if !prims.is_registered(primitive_id) {
            prims.register_mesh(primitive_id, name.clone(), mesh);
            registered_parts += 1;
            registered_vertices += vertex_count;
            registered_indices += index_count;
            newengine_ulog_api::ulog::debug!(
                "game-ready: ydd drawable mesh registered source='{}' asset='{}' part='{}' material='{}' vertices={} indices={}",
                prefab.source,
                logical_asset,
                name,
                material_slot,
                vertex_count,
                index_count
            );
        }
        let (material_id, color) = material_for_slot(
            &material_slot,
            material_ref.as_deref(),
            materials,
            material_specs,
            palette,
        );
        out.push(RuntimePrefabMeshPart {
            primitive_id,
            material_slot,
            material_id,
            color,
        });
    }
    if registered_parts > 0 {
        newengine_ulog_api::ulog::info!(
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

pub(super) fn spawn_runtime_ydd_prefab_instance(
    world: &mut newengine_ecs::World,
    prims: &PrimitiveRegistry,
    mats: &MaterialRegistry,
    root: EntityId,
    parts: &[RuntimePrefabMeshPart],
    placement: TreePlacement,
    render_options: &newengine_model_domain_api::MeshRenderOptions,
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
                name: &format!(
                    "Foliage/TreeAnimate-{}/{}-{part_index}",
                    placement.index, part.material_slot
                ),
                position: placement.position,
                scale,
                color: part.color,
                render_options: render_options.clone(),
            },
        );
        if let Some(t) = world.get_mut_tracked::<Transform>(entity) {
            t.rotation = yaw;
        }
    }
}

fn bake_foliage_mesh(
    prims: &PrimitiveRegistry,
    part: &RuntimePrefabMeshPart,
    placements: &[TreePlacement],
) -> Result<PrimitiveMesh, String> {
    let source = prims.build_mesh(part.primitive_id).map_err(|error| {
        format!(
            "foliage source primitive unavailable id={:?} slot='{}' err='{}'",
            part.primitive_id, part.material_slot, error
        )
    })?;
    let vertex_capacity = source
        .vertices
        .len()
        .checked_mul(placements.len())
        .ok_or_else(|| "foliage vertex capacity overflow".to_owned())?;
    let index_capacity = source
        .indices
        .len()
        .checked_mul(placements.len())
        .ok_or_else(|| "foliage index capacity overflow".to_owned())?;
    let mut mesh = PrimitiveMesh {
        vertices: Vec::with_capacity(vertex_capacity),
        indices: Vec::with_capacity(index_capacity),
        bounds_center: Vec3::ZERO,
        bounds_radius: 0.001,
    };

    for placement in placements {
        let base_vertex = u32::try_from(mesh.vertices.len())
            .map_err(|_| "foliage batch exceeds u32 vertex addressing".to_owned())?;
        let yaw = Quat::from_rotation_y(placement.yaw);
        let uniform_scale = placement.scale.max(0.001);
        for vertex in &source.vertices {
            let local_position = Vec3::new(vertex.pos[0], vertex.pos[1], vertex.pos[2]);
            let local_normal = Vec3::new(vertex.nrm[0], vertex.nrm[1], vertex.nrm[2]);
            let world_position = placement.position + yaw * (local_position * uniform_scale);
            let world_normal = (yaw * local_normal).normalize_or_zero();
            mesh.vertices.push(PrimitiveVertex {
                pos: [world_position.x, world_position.y, world_position.z],
                nrm: [world_normal.x, world_normal.y, world_normal.z],
                uv: vertex.uv,
            });
        }
        for &index in &source.indices {
            mesh.indices
                .push(base_vertex.checked_add(index).ok_or_else(|| {
                    "foliage batch index overflow while rebasing source mesh".to_owned()
                })?);
        }
    }

    recompute_ydd_mesh_bounds(&mut mesh);
    Ok(mesh)
}

fn spawn_runtime_ydd_prefab_batch(
    world: &mut newengine_ecs::World,
    prims: &mut PrimitiveRegistry,
    mats: &MaterialRegistry,
    root: EntityId,
    prefab: &GameReadyPrefabSpec,
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
            "game-ready.foliage.static-batch:{}:{}:{}",
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
            "game-ready foliage batch part: prefab='{}' slot='{}' placements={} vertices={} triangles={} primitive={:?}",
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

pub(crate) fn spawn_foliage_prefabs(
    world: &mut newengine_ecs::World,
    prims: &mut PrimitiveRegistry,
    mats: &MaterialRegistry,
    root: EntityId,
    terrain: EntityId,
    materials: DemoMaterials,
    material_specs: &GameReadyMaterialSetSpec,
    palette: &GameReadyPaletteSpec,
    foliage: &GameReadyFoliageSpec,
    prefabs: &[GameReadyPrefabSpec],
    player_start: Vec3,
) {
    let Some(prefab) = choose_foliage_prefab(prefabs, &foliage.prefab) else {
        if foliage.enabled {
            newengine_ulog_api::ulog::warn!(
                "game-ready: foliage enabled but prefab id='{}' is not declared or disabled",
                foliage.prefab
            );
        }
        return;
    };

    let runtime_parts = match ensure_runtime_prefab_parts(
        prims,
        prefab,
        materials,
        material_specs,
        palette,
    ) {
        Ok(parts) => parts,
        Err(e) => {
            newengine_ulog_api::ulog::error!(
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
    if count == 0 {
        log_foliage_prefab_placement(
            &prefab.id,
            &prefab.source,
            &prefab.proxy,
            "static_baked_batch",
            runtime_parts.len(),
            0,
            foliage.max_count,
            foliage.grid_min,
            foliage.grid_max,
            foliage.spacing,
        );
        return;
    }

    match spawn_runtime_ydd_prefab_batch(
        world,
        prims,
        mats,
        root,
        prefab,
        &runtime_parts,
        &placements,
        &foliage.render_options,
    ) {
        Ok(batch_parts) => {
            newengine_ulog_api::ulog::info!(
                "game-ready foliage batching: prefab='{}' placements={} source_parts={} ecs_render_entities={} reduction={:.1}x policy='static authored foliage baked into one mesh per material slot'",
                prefab.id,
                count,
                runtime_parts.len(),
                batch_parts,
                (count.saturating_mul(runtime_parts.len())) as f32 / batch_parts.max(1) as f32,
            );
        }
        Err(error) => {
            newengine_ulog_api::ulog::warn!(
                "game-ready foliage batching failed prefab='{}' err='{}'; falling back to per-placement entities",
                prefab.id,
                error,
            );
            for placement in placements {
                spawn_runtime_ydd_prefab_instance(
                    world,
                    &*prims,
                    mats,
                    root,
                    &runtime_parts,
                    placement,
                    &foliage.render_options,
                );
            }
        }
    }

    log_foliage_prefab_placement(
        &prefab.id,
        &prefab.source,
        &prefab.proxy,
        "static_baked_batch",
        runtime_parts.len(),
        count,
        foliage.max_count,
        foliage.grid_min,
        foliage.grid_max,
        foliage.spacing,
    );
}
