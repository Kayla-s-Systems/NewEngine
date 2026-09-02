pub(super) fn ensure_runtime_prefab_parts(
    prims: &mut PrimitiveRegistry,
    mats: &MaterialRegistry,
    prefab: &AuthoredWorldPlacementSpec,
    materials: AuthoredEnvironmentMaterials,
    material_specs: &AuthoredEnvironmentMaterialSetSpec,
    palette: &AuthoredEnvironmentPaletteSpec,
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
                "authored-environment: ydd drawable mesh registered source='{}' asset='{}' part='{}' material='{}' vertices={} indices={}",
                prefab.source,
                logical_asset,
                name,
                material_slot,
                vertex_count,
                index_count
            );
        }
        let (material_id, color) = material_for_slot(
            mats,
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
            "authored-environment: ydd drawable registered source='{}' asset='{}' parts={} vertices={} indices={}",
            prefab.source,
            logical_asset,
            registered_parts,
            registered_vertices,
            registered_indices,
        );
    }
    Ok(out)
}

fn spawn_foliage_collision_proxies(
    world: &mut newengine_ecs::World,
    root: EntityId,
    placements: &[TreePlacement],
    foliage: &AuthoredFoliageSpec,
) -> usize {
    if !foliage.collision_enabled || placements.is_empty() {
        return 0;
    }
    let mut spawned = 0usize;
    for placement in placements {
        let scale = placement.scale.max(0.05);
        let yaw = Quat::from_rotation_y(placement.yaw);
        let local_center = foliage.collision_center * scale;
        let entity = spawn_named(world, format!("Foliage/Collision/Tree-{}", placement.index));
        let _ = set_parent(world, entity, Some(root));
        let _ = world.insert(
            entity,
            Transform {
                position: placement.position + yaw * local_center,
                rotation: yaw,
                scale: Vec3::ONE,
            },
        );
        let shape = newengine_engine_runtime::gameplay::CollisionShapeDesc::Capsule {
            radius: (foliage.collision_radius * scale).max(0.05),
            half_height: (foliage.collision_half_height * scale).max(0.05),
        };
        let _ = world.insert(
            entity,
            newengine_engine_runtime::gameplay::PhysicsBodyDesc::static_solid(shape),
        );
        spawned = spawned.saturating_add(1);
    }
    newengine_ulog_api::ulog::info!(
        "authored-environment foliage collision: proxies={} shape='capsule' radius={:.3} half_height={:.3} center=({:.3},{:.3},{:.3}) policy='SpeedTree authored trunk collision -> per-instance static physics proxy'",
        spawned,
        foliage.collision_radius,
        foliage.collision_half_height,
        foliage.collision_center.x,
        foliage.collision_center.y,
        foliage.collision_center.z,
    );
    spawned
}

pub(super) fn spawn_runtime_ydd_prefab_instance(
    world: &mut newengine_ecs::World,
    prims: &PrimitiveRegistry,
    mats: &MaterialRegistry,
    root: EntityId,
    parts: &[RuntimePrefabMeshPart],
    placement: TreePlacement,
    render_options: &newengine_model_domain_api::MeshRenderOptions,
    foliage_runtime: &newengine_model_domain_api::FoliageInstanceRuntime,
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
        let _ = world.insert(entity, *foliage_runtime);
    }
}
