use super::materials::{register_authored_prefab_material, resolve_prefab_part_material};
use super::*;
use newengine_materials::api::MaterialRegistryApi;
use newengine_model_runtime::ydd_runtime::DecodedRuntimeYddMeshPart as DecodedPrefabMeshPart;
use newengine_physics_contracts::{CollisionShapeDesc, PhysicsBodyDesc};
use newengine_sim::{AngularVelocity, Velocity};
use std::time::Instant;

fn authored_physics_surface(
    prefab: &AuthoredWorldPlacementSpec,
) -> Option<newengine_engine_runtime::gameplay::PhysicsSurface> {
    let id = prefab.surface_id.trim().to_owned();
    let event_bindings = prefab
        .surface_events
        .iter()
        .filter_map(|(signal, event_id)| {
            let signal = signal.trim().to_owned();
            let event_id = event_id.trim().to_owned();
            (!signal.is_empty() && !event_id.is_empty()).then_some((signal, event_id))
        })
        .collect::<std::collections::BTreeMap<_, _>>();
    if id.is_empty() && event_bindings.is_empty() {
        None
    } else {
        Some(newengine_engine_runtime::gameplay::PhysicsSurface { id, event_bindings })
    }
}

fn attach_authored_physics_surface(
    world: &mut newengine_ecs::World,
    entity: EntityId,
    prefab: &AuthoredWorldPlacementSpec,
) {
    if let Some(surface) = authored_physics_surface(prefab) {
        newengine_ulog_api::ulog::info!(
            "authored physics surface attached entity={} placement='{}' surface='{}' events=[{}] ground_placement={}",
            entity.stable_u64(),
            prefab.id,
            surface.id,
            surface
                .event_bindings
                .iter()
                .map(|(signal, event)| format!("{signal}->{event}"))
                .collect::<Vec<_>>()
                .join(","),
            prefab.ground_placement_surface,
        );
        let _ = world.insert(entity, surface);
    }
    if let Some(ballistic) = prefab.ballistic_material {
        let _ = world.insert(entity, ballistic.sanitized());
    }
}

#[inline]
fn ensure_ydd_prefab_source(prefab: &AuthoredWorldPlacementSpec, role: &str) -> Result<(), String> {
    newengine_assets_api::require_asset_reference_extension(&prefab.source, &["ydd"], true)
        .map(|_| ())
        .map_err(|error| {
            format!(
                "{role} world prefab id='{}' source='{}' rejected: {error}",
                prefab.id, prefab.source
            )
        })
}

fn resolve_unambiguous_prefab_impact_material(
    mats: &MaterialRegistry,
    authored_material: Option<MaterialId>,
    decoded: &[DecodedPrefabMeshPart],
) -> Result<Option<MaterialId>, String> {
    let mut resolved = None;
    for part in decoded {
        let has_part_material = part
            .material_ref
            .as_deref()
            .map(str::trim)
            .is_some_and(|value| !value.is_empty());
        if !has_part_material && authored_material.is_none() {
            continue;
        }
        let (material, _) = resolve_prefab_part_material(
            mats,
            authored_material,
            &part.material_slot,
            part.material_ref.as_deref(),
        )?;
        match resolved {
            None => resolved = Some(material),
            Some(existing) if existing == material => {}
            Some(_) => return Ok(None),
        }
    }
    Ok(resolved.or(authored_material))
}

fn attach_unambiguous_impact_material(
    world: &mut newengine_ecs::World,
    entity: EntityId,
    mats: &MaterialRegistry,
    authored_material: Option<MaterialId>,
    decoded: &[DecodedPrefabMeshPart],
) -> Result<(), String> {
    if let Some(material) =
        resolve_unambiguous_prefab_impact_material(mats, authored_material, decoded)?
    {
        let _ = world.insert(
            entity,
            newengine_engine_runtime::scene_bridge::PrimitiveMaterialBase { id: material },
        );
    }
    Ok(())
}

fn attach_authored_map_placement(
    world: &mut newengine_ecs::World,
    entity: EntityId,
    prefab: &AuthoredWorldPlacementSpec,
) {
    if prefab.authored_map_ref.trim().is_empty() || prefab.authored_placement_id.trim().is_empty() {
        return;
    }
    let source = if prefab.authored_discrete_placement {
        newengine_world_authoring_api::AuthoredMapPlacementSource::DiscretePlacement
    } else {
        newengine_world_authoring_api::AuthoredMapPlacementSource::ProfilePrefab
    };
    let _ = world.insert(
        entity,
        newengine_world_authoring_api::AuthoredMapPlacement::new(
            prefab.authored_map_ref.clone(),
            prefab.authored_placement_id.clone(),
            source,
            prefab.authored_primary
                && !prefab
                    .proxy
                    .trim()
                    .eq_ignore_ascii_case("world_collision_ydd"),
        ),
    );
}

pub(super) fn spawn_collision_ydd_prefab_from_decoded(
    world: &mut newengine_ecs::World,
    mats: &MaterialRegistry,
    parent: EntityId,
    prefab: &AuthoredWorldPlacementSpec,
    decoded: &[DecodedPrefabMeshPart],
) -> Result<(u32, u64), String> {
    ensure_ydd_prefab_source(prefab, "collision")?;
    let authored_material = register_authored_prefab_material(mats, prefab)?;
    let mut vertices = Vec::<[f32; 3]>::new();
    let mut triangles = Vec::<[u32; 3]>::new();
    let mut part_count = 0u32;
    for part in decoded {
        let base = u32::try_from(vertices.len())
            .map_err(|_| "collision mesh exceeds u32 vertex addressing".to_owned())?;
        for vertex in &part.mesh.vertices {
            vertices.push([
                vertex.pos[0] * prefab.scale.x,
                vertex.pos[1] * prefab.scale.y,
                vertex.pos[2] * prefab.scale.z,
            ]);
        }
        for triangle in part.mesh.indices.as_chunks::<3>().0 {
            triangles.push([
                base.checked_add(triangle[0])
                    .ok_or("collision index overflow")?,
                base.checked_add(triangle[1])
                    .ok_or("collision index overflow")?,
                base.checked_add(triangle[2])
                    .ok_or("collision index overflow")?,
            ]);
        }
        part_count = part_count.saturating_add(1);
    }
    let triangle_count = triangles.len() as u64;
    let collider =
        newengine_engine_runtime::gameplay::StaticMeshCollider::new(vertices, triangles)?
            .with_material(0.94, 0.0);
    let local_bounds = collider.local_bounds;
    let vertex_count = collider.vertices.len();
    let entity = spawn_named(world, format!("World/Collision/{}", prefab.id));
    let _ = set_parent(world, entity, Some(parent));
    let rotation = Quat::from_euler(
        EulerRot::YXZ,
        prefab.rotation_ypr.x,
        prefab.rotation_ypr.y,
        prefab.rotation_ypr.z,
    );
    let _ = world.insert(
        entity,
        Transform {
            position: prefab.position,
            rotation,
            scale: Vec3::ONE,
        },
    );
    attach_authored_map_placement(world, entity, prefab);
    if prefab.authored_discrete_placement && !prefab.authored_primary {
        let _ = world.insert(
            entity,
            newengine_world_authoring_api::AuthoredMapPlacementReplicaScaleState {
                last_authored_scale: prefab.scale,
            },
        );
    }
    let _ = world.insert(entity, Bounds::from_local_aabb(local_bounds));
    let _ = world.insert(entity, collider);
    attach_authored_physics_surface(world, entity, prefab);
    attach_unambiguous_impact_material(world, entity, mats, authored_material, decoded)?;
    if prefab.ground_placement_surface {
        let _ = world.insert(entity, crate::AuthoredGroundPlacementSurface);
    }
    newengine_ulog_api::ulog::debug!(
        "static world collision spawned id='{}' source='{}' entity={:?} parts={} vertices={} triangles={} position={:?} rotation_ypr={:?} scale_baked={:?} bounds_min={:?} bounds_max={:?}",
        prefab.id,
        prefab.source,
        entity,
        part_count,
        vertex_count,
        triangle_count,
        prefab.position,
        prefab.rotation_ypr,
        prefab.scale,
        local_bounds.min,
        local_bounds.max,
    );
    Ok((part_count, triangle_count))
}

pub(super) fn spawn_box_collision_ydd_prefab_from_decoded(
    world: &mut newengine_ecs::World,
    mats: &MaterialRegistry,
    parent: EntityId,
    prefab: &AuthoredWorldPlacementSpec,
    decoded: &[DecodedPrefabMeshPart],
) -> Result<(u32, u64), String> {
    ensure_ydd_prefab_source(prefab, "box collision")?;
    let authored_material = register_authored_prefab_material(mats, prefab)?;
    let half_extents = dynamic_prefab_half_extents(decoded, prefab.scale);
    let entity = spawn_named(world, format!("World/Collision/{}", prefab.id));
    let _ = set_parent(world, entity, Some(parent));
    let rotation = Quat::from_euler(
        EulerRot::YXZ,
        prefab.rotation_ypr.x,
        prefab.rotation_ypr.y,
        prefab.rotation_ypr.z,
    );
    let _ = world.insert(
        entity,
        Transform {
            position: prefab.position,
            rotation,
            scale: Vec3::ONE,
        },
    );
    attach_authored_map_placement(world, entity, prefab);
    if prefab.authored_discrete_placement && !prefab.authored_primary {
        let _ = world.insert(
            entity,
            newengine_world_authoring_api::AuthoredMapPlacementReplicaScaleState {
                last_authored_scale: prefab.scale,
            },
        );
    }
    let local_bounds = newengine_bounds::Aabb::from_center_half_extents(Vec3::ZERO, half_extents);
    let _ = world.insert(entity, Bounds::from_local_aabb(local_bounds));
    let mut body = PhysicsBodyDesc::static_solid(CollisionShapeDesc::Box {
        half_extents: [half_extents.x, half_extents.y, half_extents.z],
    });
    body.material.friction = 0.94;
    body.material.restitution = 0.0;
    let _ = world.insert(entity, body);
    attach_authored_physics_surface(world, entity, prefab);
    attach_unambiguous_impact_material(world, entity, mats, authored_material, decoded)?;
    let part_count = decoded.len() as u32;
    let triangle_count = decoded
        .iter()
        .map(|part| (part.mesh.indices.len() / 3) as u64)
        .sum();
    newengine_ulog_api::ulog::info!(
        "static box collision spawned id='{}' source='{}' entity={:?} half_extents={:?} position={:?}",
        prefab.id,
        prefab.source,
        entity,
        half_extents,
        prefab.position,
    );
    Ok((part_count, triangle_count))
}

pub(super) fn dynamic_prefab_half_extents(decoded: &[DecodedPrefabMeshPart], scale: Vec3) -> Vec3 {
    let mut min = Vec3::splat(f32::INFINITY);
    let mut max = Vec3::splat(f32::NEG_INFINITY);
    for part in decoded {
        for vertex in &part.mesh.vertices {
            let p = Vec3::new(
                vertex.pos[0] * scale.x,
                vertex.pos[1] * scale.y,
                vertex.pos[2] * scale.z,
            );
            min = min.min(p);
            max = max.max(p);
        }
    }
    if !min.is_finite() || !max.is_finite() {
        return Vec3::new(
            scale.x.abs().max(0.5),
            scale.y.abs().max(0.5),
            scale.z.abs().max(0.5),
        );
    }
    Vec3::new(
        min.x.abs().max(max.x.abs()).max(0.05),
        min.y.abs().max(max.y.abs()).max(0.05),
        min.z.abs().max(max.z.abs()).max(0.05),
    )
}

pub(super) fn spawn_dynamic_ydd_prefab_from_decoded(
    world: &mut newengine_ecs::World,
    prims: &mut PrimitiveRegistry,
    mats: &MaterialRegistry,
    parent: EntityId,
    prefab: &AuthoredWorldPlacementSpec,
    decoded: &[DecodedPrefabMeshPart],
) -> Result<(u32, u64), String> {
    ensure_ydd_prefab_source(prefab, "dynamic")?;

    let authored_material = register_authored_prefab_material(mats, prefab)?;
    let root = spawn_named(world, format!("World/Dynamic/{}", prefab.id));
    let _ = set_parent(world, root, Some(parent));
    let rotation = Quat::from_euler(
        EulerRot::YXZ,
        prefab.rotation_ypr.x,
        prefab.rotation_ypr.y,
        prefab.rotation_ypr.z,
    );
    let _ = world.insert(
        root,
        Transform {
            position: prefab.position,
            rotation,
            scale: prefab.scale,
        },
    );
    let half_extents = dynamic_prefab_half_extents(decoded, prefab.scale);
    attach_authored_map_placement(world, root, prefab);
    newengine_engine_runtime::gameplay::attach_scene_object_core(
        world,
        root,
        prefab.position,
        half_extents,
    );
    let mut body = PhysicsBodyDesc::dynamic_solid(CollisionShapeDesc::Box {
        half_extents: [half_extents.x, half_extents.y, half_extents.z],
    });
    body.material.friction = 0.72;
    body.material.restitution = 0.08;
    body.material.density = 0.85;
    let _ = world.insert(root, body);
    attach_authored_physics_surface(world, root, prefab);
    attach_unambiguous_impact_material(world, root, mats, authored_material, decoded)?;
    let _ = world.insert(root, Velocity(Vec3::ZERO));
    let _ = world.insert(root, AngularVelocity(Vec3::ZERO));

    let mut part_count = 0u32;
    let mut triangle_count = 0u64;
    for (part_index, part) in decoded.iter().enumerate() {
        let primitive_id = part.primitive_id;
        triangle_count = triangle_count.saturating_add((part.mesh.indices.len() / 3) as u64);
        if !prims.is_registered(primitive_id) {
            prims.register_mesh(primitive_id, part.name.clone(), part.mesh.clone());
        }
        let (material_id, render_options) = resolve_prefab_part_material(
            mats,
            authored_material,
            &part.material_slot,
            part.material_ref.as_deref(),
        )?;
        let _ = spawn_game_primitive(
            world,
            &*prims,
            mats,
            PrimitiveSpawnSpec {
                parent: root,
                primitive_id,
                material_id,
                name: &format!(
                    "World/Dynamic/{}/{}-{part_index}",
                    prefab.id, part.material_slot
                ),
                position: Vec3::ZERO,
                scale: Vec3::ONE,
                color: [1.0, 1.0, 1.0, 1.0],
                render_options,
            },
        );
        part_count = part_count.saturating_add(1);
    }
    newengine_ulog_api::ulog::info!(
        "dynamic world prefab spawned id='{}' source='{}' entity={:?} half_extents={:?} parts={} triangles={}",
        prefab.id,
        prefab.source,
        root,
        half_extents,
        part_count,
        triangle_count,
    );
    Ok((part_count, triangle_count))
}

pub(super) fn spawn_static_ydd_prefab_from_decoded(
    world: &mut newengine_ecs::World,
    prims: &mut PrimitiveRegistry,
    mats: &MaterialRegistry,
    parent: EntityId,
    prefab: &AuthoredWorldPlacementSpec,
    decoded: &[DecodedPrefabMeshPart],
) -> Result<(u32, u64), String> {
    ensure_ydd_prefab_source(prefab, "static")?;

    let admission_started = Instant::now();
    let material_started = Instant::now();
    let authored_material = register_authored_prefab_material(mats, prefab)?;
    let mut material_ms = material_started.elapsed().as_secs_f64() * 1000.0;
    let setup_started = Instant::now();
    let root = spawn_named(world, format!("World/Static/{}", prefab.id));
    let _ = set_parent(world, root, Some(parent));
    let _ = world.insert(
        root,
        Transform {
            position: prefab.position,
            rotation: Quat::from_euler(
                EulerRot::YXZ,
                prefab.rotation_ypr.x,
                prefab.rotation_ypr.y,
                prefab.rotation_ypr.z,
            ),
            scale: prefab.scale,
        },
    );
    attach_authored_map_placement(world, root, prefab);
    newengine_engine_runtime::gameplay::attach_scene_object_core(
        world,
        root,
        prefab.position,
        Vec3::new(
            400.0 * prefab.scale.x.abs().max(0.001),
            100.0 * prefab.scale.y.abs().max(0.001),
            400.0 * prefab.scale.z.abs().max(0.001),
        ),
    );

    let setup_ms = setup_started.elapsed().as_secs_f64() * 1000.0;
    let mut primitive_register_ms = 0.0f64;
    let mut entity_spawn_ms = 0.0f64;
    let mut verify_ms = 0.0f64;
    let mut part_count = 0u32;
    let mut triangle_count = 0u64;
    for (part_index, part) in decoded.iter().enumerate() {
        let primitive_id = part.primitive_id;
        let vertex_count = part.mesh.vertices.len();
        let index_count = part.mesh.indices.len();
        triangle_count = triangle_count.saturating_add((index_count / 3) as u64);
        if !prims.is_registered(primitive_id) {
            let started = Instant::now();
            prims.register_mesh(primitive_id, part.name.clone(), part.mesh.clone());
            primitive_register_ms += started.elapsed().as_secs_f64() * 1000.0;
        }
        let started = Instant::now();
        let (material_id, render_options) = resolve_prefab_part_material(
            mats,
            authored_material,
            &part.material_slot,
            part.material_ref.as_deref(),
        )?;
        material_ms += started.elapsed().as_secs_f64() * 1000.0;
        let started = Instant::now();
        let part_entity = spawn_game_primitive(
            world,
            &*prims,
            mats,
            PrimitiveSpawnSpec {
                parent: root,
                primitive_id,
                material_id,
                name: &format!(
                    "World/Static/{}/{}-{part_index}",
                    prefab.id, part.material_slot
                ),
                position: Vec3::ZERO,
                scale: Vec3::ONE,
                color: [1.0, 1.0, 1.0, 1.0],
                render_options,
            },
        );
        entity_spawn_ms += started.elapsed().as_secs_f64() * 1000.0;
        let started = Instant::now();
        let applied_ref = world
            .get::<newengine_materials::MaterialRef>(part_entity)
            .copied()
            .ok_or_else(|| {
                format!(
                    "static world part prefab='{}' slot='{}' lost MaterialRef after spawn",
                    prefab.id, part.material_slot
                )
            })?;
        let applied = mats.resolve(applied_ref.id).ok_or_else(|| {
            format!(
                "static world part prefab='{}' slot='{}' MaterialRef={:?} is not resolvable in scene material registry",
                prefab.id, part.material_slot, applied_ref.id
            )
        })?;
        newengine_ulog_api::ulog::debug!(
            "static world material binding verified prefab='{}' slot='{}' entity={:?} material_ref={:?} base={:?} normal={:?} roughness={:?}",
            prefab.id,
            part.material_slot,
            part_entity,
            applied_ref.id,
            applied.textures.base_color_texture,
            applied.textures.normal_texture,
            applied.textures.roughness_texture,
        );
        verify_ms += started.elapsed().as_secs_f64() * 1000.0;
        // Static imported geometry is currently visual-only. The procedural terrain
        // remains the authoritative walkable collision surface; this prevents a
        // single coarse collider from enclosing the entire winding road mesh.
        part_count = part_count.saturating_add(1);
        newengine_ulog_api::ulog::debug!(
            "static world part spawned prefab='{}' part='{}' vertices={} triangles={} material_id={:?}",
            prefab.id,
            part.material_slot,
            vertex_count,
            index_count / 3,
            material_id,
        );
    }

    let total_ms = admission_started.elapsed().as_secs_f64() * 1000.0;
    if total_ms >= 50.0 {
        newengine_ulog_api::ulog::info!(
            "static world admission slow id='{}' source='{}' parts={} triangles={} total_ms={:.2} setup_ms={:.2} material_ms={:.2} primitive_register_ms={:.2} entity_spawn_ms={:.2} verify_ms={:.2}",
            prefab.id,
            prefab.source,
            part_count,
            triangle_count,
            total_ms,
            setup_ms,
            material_ms,
            primitive_register_ms,
            entity_spawn_ms,
            verify_ms,
        );
    }
    Ok((part_count, triangle_count))
}
