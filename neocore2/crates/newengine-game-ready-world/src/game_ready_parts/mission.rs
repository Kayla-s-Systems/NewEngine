use super::foliage::{decode_runtime_ydd_prefab, terrain_height, DecodedPrefabMeshPart};
use super::*;
use crate::content::GameReadyMissionPickupSpec;

const MISSION_MATERIAL_LIBRARY: &str = newengine_game_data::MISSION_MATERIAL_LIBRARY;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct GameReadyMissionSpawnSummary {
    /// Mission-objective pickups only (relay cores, etc.).
    pub pickups: u32,
    /// Inventory-backed authored world pickups. These never affect mission-core totals.
    pub item_pickups: u32,
    pub targets: u32,
    pub hazards: u32,
    pub goals: u32,
}

#[derive(Clone, Debug)]
struct DeferredWorldItemPickup {
    parent: EntityId,
    terrain: EntityId,
    spec: GameReadyMissionPickupSpec,
    attempts: u32,
}

#[derive(Clone, Debug, Default)]
struct DeferredWorldItemPickups {
    pending: Vec<DeferredWorldItemPickup>,
}

#[derive(Clone, Copy, Debug)]
struct MissionMaterials {
    core: MaterialId,
    target: MaterialId,
    hazard: MaterialId,
    goal: MaterialId,
}

fn mission_material_spec(entry: &str) -> GameReadyMaterialSpec {
    GameReadyMaterialSpec {
        asset: Some(format!("{MISSION_MATERIAL_LIBRARY}@{entry}")),
        base_color_texture: None,
        normal_texture: None,
        roughness_texture: None,
        uv_scale: [1.0, 1.0],
        uv_offset: [0.0, 0.0],
        roughness: 0.3,
        normal_scale: 0.0,
        occlusion_strength: 1.0,
    }
}

fn register_mission_materials(mats: &MaterialRegistry) -> MissionMaterials {
    let core_spec = mission_material_spec("mission_core");
    let target_spec = mission_material_spec("mission_target");
    let hazard_spec = mission_material_spec("mission_hazard");
    let goal_spec = mission_material_spec("mission_goal");

    MissionMaterials {
        core: register_material(
            mats,
            "FPS/Mission/Core",
            [0.04, 0.62, 1.0, 1.0],
            [0.02, 0.55, 1.0],
            3.2,
            MaterialFlags::DOUBLE_SIDED,
            &core_spec,
        ),
        target: register_material(
            mats,
            "FPS/Mission/Target",
            [1.0, 0.18, 0.04, 1.0],
            [0.72, 0.06, 0.01],
            1.5,
            MaterialFlags::CAST_SHADOWS.union(MaterialFlags::RECEIVE_SHADOWS),
            &target_spec,
        ),
        hazard: register_material(
            mats,
            "FPS/Mission/Hazard",
            [0.96, 0.02, 0.08, 1.0],
            [1.0, 0.01, 0.04],
            3.8,
            MaterialFlags::DOUBLE_SIDED,
            &hazard_spec,
        ),
        goal: register_material(
            mats,
            "FPS/Mission/Goal",
            [0.08, 1.0, 0.34, 1.0],
            [0.04, 1.0, 0.22],
            3.4,
            MaterialFlags::DOUBLE_SIDED,
            &goal_spec,
        ),
    }
}

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

fn rotated_box_half_height(half_extents: Vec3, rotation: Quat) -> f32 {
    let x = rotation * Vec3::X;
    let y = rotation * Vec3::Y;
    let z = rotation * Vec3::Z;
    (x.y.abs() * half_extents.x + y.y.abs() * half_extents.y + z.y.abs() * half_extents.z).max(0.01)
}

fn decoded_model_center(decoded: &[DecodedPrefabMeshPart]) -> Result<Vec3, String> {
    let mut min = Vec3::splat(f32::INFINITY);
    let mut max = Vec3::splat(f32::NEG_INFINITY);
    for part in decoded {
        for vertex in &part.mesh.vertices {
            let point = Vec3::new(vertex.pos[0], vertex.pos[1], vertex.pos[2]);
            min = min.min(point);
            max = max.max(point);
        }
    }
    if !min.is_finite() || !max.is_finite() {
        return Err("world-item YDD produced no finite geometry bounds".to_owned());
    }
    Ok((min + max) * 0.5)
}

fn register_world_item_part_material(
    mats: &MaterialRegistry,
    pickup_id: &str,
    part_index: usize,
    part: &DecodedPrefabMeshPart,
) -> MaterialId {
    let spec = GameReadyMaterialSpec {
        asset: part.material_ref.clone(),
        base_color_texture: None,
        normal_texture: None,
        roughness_texture: None,
        uv_scale: [1.0, 1.0],
        uv_offset: [0.0, 0.0],
        roughness: 0.72,
        normal_scale: 1.0,
        occlusion_strength: 1.0,
    };
    let diagnostic_color = match part.material_slot.as_str() {
        "m00" => [0.10, 0.13, 0.10, 1.0], // dark synthetic/polymer furniture
        "m01" => [0.07, 0.08, 0.09, 1.0], // blued/gunmetal receiver and barrel
        _ => [0.12, 0.13, 0.13, 1.0],
    };
    register_material(
        mats,
        &format!(
            "WorldItem/{pickup_id}/Part{part_index}:{}",
            part.material_slot
        ),
        diagnostic_color,
        [0.0, 0.0, 0.0],
        1.0,
        MaterialFlags::CAST_SHADOWS.union(MaterialFlags::RECEIVE_SHADOWS),
        &spec,
    )
}

fn bind_world_item_model_from_decoded(
    world: &mut newengine_ecs::World,
    prims: &mut PrimitiveRegistry,
    mats: &MaterialRegistry,
    owner: EntityId,
    pickup_id: &str,
    authored_scale: Vec3,
    decoded: &[DecodedPrefabMeshPart],
) -> Result<u32, String> {
    let presentation = world
        .get::<newengine_engine_runtime::gameplay::WorldItemPresentation>(owner)
        .cloned()
        .ok_or_else(|| "world item has no presentation component".to_owned())?;
    let visual_root = presentation.visual_entity;
    if !world.exists(visual_root) {
        return Err("world item visual root no longer exists".to_owned());
    }
    let center = decoded_model_center(decoded)?;

    let base_scale = presentation.scale;
    if let Some(transform) = world.get_mut_tracked::<Transform>(visual_root) {
        transform.position = Vec3::ZERO;
        transform.rotation = Quat::IDENTITY;
        transform.scale = Vec3::new(
            base_scale.x * authored_scale.x,
            base_scale.y * authored_scale.y,
            base_scale.z * authored_scale.z,
        );
    }

    let mut spawned = 0u32;
    for (part_index, part) in decoded.iter().enumerate() {
        if !prims.is_registered(part.primitive_id) {
            prims.register_mesh(part.primitive_id, part.name.clone(), part.mesh.clone());
        }
        let material_id = register_world_item_part_material(mats, pickup_id, part_index, part);
        let child = spawn_game_primitive(
            world,
            &*prims,
            mats,
            PrimitiveSpawnSpec {
                parent: visual_root,
                primitive_id: part.primitive_id,
                material_id,
                name: &format!("WorldItem/{pickup_id}/{}-{part_index}", part.material_slot),
                position: -center,
                scale: Vec3::ONE,
                color: [1.0, 1.0, 1.0, 1.0],
                render_options: newengine_model_domain_api::MeshRenderOptions::world_opaque(),
            },
        );
        let _ = world.insert(
            child,
            newengine_engine_runtime::gameplay::WorldItemVisualPart { owner },
        );
        let _ = world.insert(
            child,
            newengine_engine_runtime::gameplay::DisplayVisibility::default(),
        );
        spawned = spawned.saturating_add(1);
    }

    if spawned == 0 {
        return Err("world-item YDD contains no renderable parts".to_owned());
    }
    // The primitive created by generic inventory code is a boot-safe fallback only.
    // Keep it until every authored YDD part has been admitted, then remove it atomically.
    let _ = world.remove::<Primitive>(visual_root);
    newengine_ulog_api::ulog::info!(
        "game-ready world item model bound id='{}' owner={:?} model='{}' parts={} center={:?} policy='ItemPickup -> YDD/NEMAT exact authored visual; fallback primitive removed after admission'",
        pickup_id,
        owner,
        presentation.model_ref.as_deref().unwrap_or(""),
        spawned,
        center,
    );
    Ok(spawned)
}

fn try_spawn_deferred_world_item(
    world: &mut newengine_ecs::World,
    prims: &mut PrimitiveRegistry,
    mats: &MaterialRegistry,
    pending: &DeferredWorldItemPickup,
) -> Result<EntityId, String> {
    let item_name = pending
        .spec
        .item
        .as_deref()
        .ok_or_else(|| "deferred item pickup has no item id".to_owned())?;
    let item = newengine_engine_runtime::gameplay::ItemId::from_name(item_name)
        .ok_or_else(|| format!("invalid authored item id '{item_name}'"))?;
    let definition = world
        .resource::<newengine_engine_runtime::gameplay::ItemCatalog>()
        .and_then(|catalog| catalog.get(item))
        .cloned()
        .ok_or_else(|| format!("item definition is not installed id='{item_name}'"))?;
    let world_definition = definition.world.clone().sanitized();

    let decoded = match world_definition.model_ref.as_deref() {
        Some(model_ref) => Some(decode_runtime_ydd_prefab(model_ref).map_err(|error| {
            format!("world-item model decode failed path='{model_ref}': {error}")
        })?),
        None => None,
    };

    let rotation = Quat::from_euler(
        EulerRot::YXZ,
        pending.spec.rotation_ypr.x,
        pending.spec.rotation_ypr.y,
        pending.spec.rotation_ypr.z,
    );
    let local_half_extents = Vec3::new(
        world_definition.pickup_half_extents[0] * pending.spec.scale.x.abs(),
        world_definition.pickup_half_extents[1] * pending.spec.scale.y.abs(),
        world_definition.pickup_half_extents[2] * pending.spec.scale.z.abs(),
    );
    let position = mission_position(
        world,
        pending.terrain,
        pending.spec.position,
        rotated_box_half_height(local_half_extents, rotation),
    );
    let entity = newengine_engine_runtime::gameplay::spawn_persistent_item_pickup(
        world,
        Some(pending.parent),
        item,
        pending.spec.quantity,
        position,
        &pending.spec.id,
        0.0,
    )?;
    if let Some(transform) = world.get_mut_tracked::<Transform>(entity) {
        transform.rotation = rotation;
    }
    if let Some(pickup) = world.get_mut::<newengine_engine_runtime::gameplay::ItemPickup>(entity) {
        pickup.auto_equip = pending.spec.auto_equip;
    }

    if let Some(decoded) = decoded.as_deref() {
        bind_world_item_model_from_decoded(
            world,
            prims,
            mats,
            entity,
            &pending.spec.id,
            pending.spec.scale,
            decoded,
        )?;
    }

    newengine_ulog_api::ulog::info!(
        "game-ready inventory pickup spawned id='{}' item='{}' entity={:?} quantity={} auto_equip={} position={:?} rotation_ypr={:?} model={:?}",
        pending.spec.id,
        item_name,
        entity,
        pending.spec.quantity,
        pending.spec.auto_equip,
        position,
        pending.spec.rotation_ypr,
        world_definition.model_ref,
    );
    Ok(entity)
}

pub(super) fn tick_deferred_item_pickups(
    world: &mut newengine_ecs::World,
    prims: &mut PrimitiveRegistry,
    mats: &MaterialRegistry,
) {
    let Some(mut queue) = world.remove_resource::<DeferredWorldItemPickups>() else {
        return;
    };
    if world
        .resource::<newengine_engine_runtime::gameplay::ItemCatalog>()
        .is_none()
    {
        world.insert_resource(queue);
        return;
    }

    let mut remaining = Vec::new();
    for mut pending in queue.pending.drain(..) {
        match try_spawn_deferred_world_item(world, prims, mats, &pending) {
            Ok(_) => {}
            Err(error) => {
                pending.attempts = pending.attempts.saturating_add(1);
                if pending.attempts == 1 || pending.attempts % 60 == 0 {
                    newengine_ulog_api::ulog::warn!(
                        "game-ready inventory pickup admission deferred id='{}' item={:?} attempt={} err='{}'",
                        pending.spec.id,
                        pending.spec.item,
                        pending.attempts,
                        error,
                    );
                }
                if pending.attempts < 300 {
                    remaining.push(pending);
                } else {
                    newengine_ulog_api::ulog::error!(
                        "game-ready inventory pickup admission abandoned id='{}' item={:?} attempts={} err='{}'",
                        pending.spec.id,
                        pending.spec.item,
                        pending.attempts,
                        error,
                    );
                }
            }
        }
    }
    if !remaining.is_empty() {
        queue.pending = remaining;
        world.insert_resource(queue);
    }
}

pub(super) fn spawn_game_ready_mission(
    world: &mut newengine_ecs::World,
    prims: &mut PrimitiveRegistry,
    mats: &MaterialRegistry,
    parent: EntityId,
    terrain: EntityId,
    mission: &GameReadyMissionSpec,
) -> GameReadyMissionSpawnSummary {
    let mut summary = GameReadyMissionSpawnSummary::default();
    let materials = register_mission_materials(mats);

    let mut deferred_items = Vec::new();
    for pickup in &mission.pickups {
        if pickup.item.is_some() {
            deferred_items.push(DeferredWorldItemPickup {
                parent,
                terrain,
                spec: pickup.clone(),
                attempts: 0,
            });
            summary.item_pickups = summary.item_pickups.saturating_add(1);
            continue;
        }

        let position = mission_position(world, terrain, pickup.position, pickup.scale.y.abs());
        let entity = spawn_mission_primitive(
            world,
            &*prims,
            mats,
            parent,
            materials.core,
            builtins::ID_SPHERE_UV,
            &format!("Mission/Pickup/{}", pickup.id),
            position,
            pickup.scale,
        );
        let _ = world.insert(
            entity,
            FpsDemoPickup {
                radius: pickup.radius,
            },
        );
        let _ = world.insert(
            entity,
            newengine_engine_runtime::gameplay::Interactable::new(format!("Collect {}", pickup.id)),
        );
        summary.pickups = summary.pickups.saturating_add(1);
    }
    if !deferred_items.is_empty() {
        let mut queue = world
            .remove_resource::<DeferredWorldItemPickups>()
            .unwrap_or_default();
        queue.pending.extend(deferred_items);
        world.insert_resource(queue);
    }

    for target in &mission.targets {
        let position = mission_position(world, terrain, target.position, target.scale.y.abs());
        let entity = spawn_mission_primitive(
            world,
            &*prims,
            mats,
            parent,
            materials.target,
            builtins::ID_CAPSULE,
            &format!("Mission/Target/{}", target.id),
            position,
            target.scale,
        );
        let shape = newengine_engine_runtime::gameplay::CollisionShapeDesc::Capsule {
            radius: target.scale.x.abs().max(target.scale.z.abs()).max(0.1),
            half_height: (target.scale.y.abs() - target.scale.x.abs()).max(0.1),
        };
        let _ = world.insert(
            entity,
            newengine_engine_runtime::gameplay::PhysicsBodyDesc::static_solid(shape),
        );
        let _ = world.insert(
            entity,
            newengine_engine_runtime::gameplay::Health::new(target.health),
        );
        let _ = world.insert(entity, FpsDemoTarget);
        summary.targets = summary.targets.saturating_add(1);
    }

    for hazard in &mission.hazards {
        let position = mission_position(world, terrain, hazard.position, hazard.scale.y.abs());
        let entity = spawn_mission_primitive(
            world,
            &*prims,
            mats,
            parent,
            materials.hazard,
            builtins::ID_CYLINDER,
            &format!("Mission/Hazard/{}", hazard.id),
            position,
            hazard.scale,
        );
        let _ = world.insert(
            entity,
            FpsDemoHazard {
                radius: hazard.radius,
            },
        );
        summary.hazards = summary.hazards.saturating_add(1);
    }

    for goal in &mission.goals {
        let position = mission_position(world, terrain, goal.position, goal.scale.y.abs() * 0.15);
        let entity = spawn_mission_primitive(
            world,
            &*prims,
            mats,
            parent,
            materials.goal,
            builtins::ID_TORUS,
            &format!("Mission/Goal/{}", goal.id),
            position,
            goal.scale,
        );
        let _ = world.insert(
            entity,
            FpsDemoGoal {
                radius: goal.radius,
            },
        );
        let _ = world.insert(
            entity,
            newengine_engine_runtime::gameplay::Interactable::new(format!(
                "Extract at {}",
                goal.id
            )),
        );
        summary.goals = summary.goals.saturating_add(1);
    }

    newengine_ulog_api::ulog::info!(
        "game-ready mission spawned: pickups={} item_pickups={} targets={} hazards={} goals={} materials='{}@mission_*' policy='mission cores stay FpsDemoPickup; item-backed Pickup -> deferred inventory ItemPickup'",
        summary.pickups,
        summary.item_pickups,
        summary.targets,
        summary.hazards,
        summary.goals,
        MISSION_MATERIAL_LIBRARY,
    );
    summary
}
