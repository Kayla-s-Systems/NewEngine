use super::foliage::{decode_runtime_ydd_prefab, terrain_height, DecodedPrefabMeshPart};
use super::*;
use crate::content::GameReadyMissionPickupSpec;

const MISSION_STREAMING_PIN_OWNER: &str = "game-ready.mission";

#[derive(Debug, Default)]
struct MissionAssetPinSet {
    leases: std::collections::BTreeMap<String, newengine_assets::AssetStreamingPinLease>,
}

fn pin_mission_asset(world: &mut newengine_ecs::World, logical_path: &str) -> Result<(), String> {
    let logical_path = logical_path.trim().replace('\\', "/");
    if logical_path.is_empty() {
        return Ok(());
    }

    let mut pins = world
        .remove_resource::<MissionAssetPinSet>()
        .unwrap_or_default();
    if pins.leases.contains_key(&logical_path) {
        world.insert_resource(pins);
        return Ok(());
    }

    let client =
        newengine_assets::AssetServiceClient::new(newengine_plugin_host::default_host_api());
    match newengine_assets::AssetStreamingPinLease::acquire(
        client,
        logical_path.clone(),
        MISSION_STREAMING_PIN_OWNER,
        newengine_assets::AssetStreamingPinClassV1::Mission,
    ) {
        Ok(lease) => {
            pins.leases.insert(logical_path, lease);
            world.insert_resource(pins);
            Ok(())
        }
        Err(error) => {
            world.insert_resource(pins);
            Err(error)
        }
    }
}

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

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct RuntimeWorldItemAdmissionState {
    attempts: u32,
}

#[derive(Clone, Copy, Debug, Default)]
struct MissionMaterials {
    core: Option<MaterialId>,
    target: Option<MaterialId>,
    hazard: Option<MaterialId>,
    goal: Option<MaterialId>,
}

fn mission_material(
    mats: &MaterialRegistry,
    role: &str,
    authored_ref: Option<&str>,
    required: bool,
) -> Result<Option<MaterialId>, String> {
    if !required {
        return Ok(None);
    }
    let reference = authored_ref
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            format!(
                "mission role '{}' requires a project-authored material reference",
                role
            )
        })?;
    let id = register_required_material_ref(
        mats,
        &format!("Mission/{role}"),
        MaterialFlags::CAST_SHADOWS.union(MaterialFlags::RECEIVE_SHADOWS),
        reference,
    )?;
    Ok(Some(id))
}

fn register_mission_materials(
    mats: &MaterialRegistry,
    mission: &GameReadyMissionSpec,
) -> Result<MissionMaterials, String> {
    Ok(MissionMaterials {
        core: mission_material(
            mats,
            "Core",
            mission.core_material.as_deref(),
            !mission.pickups.is_empty(),
        )?,
        target: mission_material(
            mats,
            "Target",
            mission.target_material.as_deref(),
            !mission.targets.is_empty(),
        )?,
        hazard: mission_material(
            mats,
            "Hazard",
            mission.hazard_material.as_deref(),
            !mission.hazards.is_empty(),
        )?,
        goal: mission_material(
            mats,
            "Goal",
            mission.goal_material.as_deref(),
            !mission.goals.is_empty(),
        )?,
    })
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

fn decoded_model_bounds(decoded: &[DecodedPrefabMeshPart]) -> Result<(Vec3, Vec3), String> {
    let mut min = Vec3::splat(f32::INFINITY);
    let mut max = Vec3::splat(f32::NEG_INFINITY);
    for part in decoded {
        for vertex in &part.mesh.vertices {
            let point = Vec3::new(vertex.pos[0], vertex.pos[1], vertex.pos[2]);
            min = min.min(point);
            max = max.max(point);
        }
    }
    let extent = max - min;
    if !min.is_finite() || !max.is_finite() || extent.x <= 0.0 || extent.y <= 0.0 || extent.z <= 0.0
    {
        return Err("world-item YDD produced invalid/non-finite geometry bounds".to_owned());
    }
    Ok((min, max))
}

#[inline]
fn world_item_material_asset(
    part_material_ref: Option<&str>,
    material_slot: &str,
    fallback_material_library: Option<&str>,
) -> Option<String> {
    match part_material_ref {
        Some(reference) if reference.contains('@') => Some(reference.trim().to_owned()),
        Some(reference) if !reference.trim().is_empty() => {
            Some(format!("{}@{}", reference.trim(), material_slot))
        }
        _ => fallback_material_library
            .map(str::trim)
            .filter(|reference| !reference.is_empty())
            .map(|reference| {
                if reference.contains('@') {
                    reference.to_owned()
                } else {
                    format!("{reference}@{material_slot}")
                }
            }),
    }
}

fn register_world_item_part_material(
    mats: &MaterialRegistry,
    pickup_id: &str,
    part_index: usize,
    part: &DecodedPrefabMeshPart,
    fallback_material_library: Option<&str>,
) -> Result<MaterialId, String> {
    let material_asset = world_item_material_asset(
        part.material_ref.as_deref(),
        &part.material_slot,
        fallback_material_library,
    );
    let spec = GameReadyMaterialSpec {
        asset: material_asset,
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
    let logical_name = format!(
        "WorldItem/{pickup_id}/Part{part_index}:{}",
        part.material_slot
    );
    let material_id = register_required_material(
        mats,
        &logical_name,
        MaterialFlags::CAST_SHADOWS.union(MaterialFlags::RECEIVE_SHADOWS),
        &spec,
    )?;
    let resolved = newengine_materials::api::MaterialRegistryApi::resolve(mats, material_id)
        .ok_or_else(|| {
            format!(
                "required world-item material disappeared after registration name='{logical_name}'"
            )
        })?;
    let mut missing = Vec::new();
    if resolved.textures.base_color_texture.is_none() {
        missing.push("base_color");
    }
    if resolved.textures.normal_texture.is_none() {
        missing.push("normal");
    }
    if resolved.textures.roughness_texture.is_none() {
        missing.push("roughness");
    }
    if !missing.is_empty() {
        return Err(format!(
            "required world-item PBR material is incomplete name='{}' asset={:?} missing={:?}",
            logical_name, spec.asset, missing
        ));
    }
    let _ = diagnostic_color; // retained as a readable slot-class diagnostic reference.
    Ok(material_id)
}

fn world_item_render_options() -> newengine_model_domain_api::MeshRenderOptions {
    let mut options = newengine_model_domain_api::MeshRenderOptions::world_opaque();
    options.shadow_policy = newengine_model_domain_api::MeshShadowPolicy::CastAndReceive;
    options
}

fn bind_world_item_model_from_decoded(
    world: &mut newengine_ecs::World,
    prims: &mut PrimitiveRegistry,
    mats: &MaterialRegistry,
    owner: EntityId,
    pickup_id: &str,
    authored_scale: Vec3,
    decoded: &[DecodedPrefabMeshPart],
    fallback_material_library: Option<&str>,
) -> Result<u32, String> {
    let presentation = world
        .get::<newengine_engine_runtime::gameplay::WorldItemPresentation>(owner)
        .cloned()
        .ok_or_else(|| "world item has no presentation component".to_owned())?;
    let visual_root = presentation.visual_entity;
    if !world.exists(visual_root) {
        return Err("world item visual root no longer exists".to_owned());
    }
    let (bounds_min, bounds_max) = decoded_model_bounds(decoded)?;
    let center = (bounds_min + bounds_max) * 0.5;

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

    // Resolve the complete authored material set before mutating the visual hierarchy. This keeps
    // late runtime admission atomic: a temporarily unavailable m01 cannot leave a duplicated m00
    // child behind for the next retry.
    let material_ids = decoded
        .iter()
        .enumerate()
        .map(|(part_index, part)| {
            register_world_item_part_material(
                mats,
                pickup_id,
                part_index,
                part,
                fallback_material_library,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;

    let mut spawned = 0u32;
    for ((part_index, part), material_id) in decoded.iter().enumerate().zip(material_ids) {
        if !prims.is_registered(part.primitive_id) {
            prims.register_mesh(part.primitive_id, part.name.clone(), part.mesh.clone());
        }
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
                render_options: world_item_render_options(),
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
        Some(model_ref) => {
            pin_mission_asset(world, model_ref).map_err(|error| {
                format!("mission model residency pin failed path='{model_ref}': {error}")
            })?;
            if let Some(material_ref) = world_definition.material_library_ref.as_deref() {
                pin_mission_asset(world, material_ref).map_err(|error| {
                    format!("mission material residency pin failed path='{material_ref}': {error}")
                })?;
            }
            Some(decode_runtime_ydd_prefab(model_ref).map_err(|error| {
                format!("world-item model decode failed path='{model_ref}': {error}")
            })?)
        }
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
            world_definition.material_library_ref.as_deref(),
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

fn scaled_world_item_half_extents(min: Vec3, max: Vec3, scale: Vec3) -> Result<Vec3, String> {
    let extent = max - min;
    if !min.is_finite()
        || !max.is_finite()
        || !scale.is_finite()
        || extent.x <= 0.0
        || extent.y <= 0.0
        || extent.z <= 0.0
    {
        return Err("world-item physical bounds are invalid".to_owned());
    }
    let scale = Vec3::new(scale.x.abs(), scale.y.abs(), scale.z.abs());
    let local_half = extent * 0.5;
    Ok(Vec3::new(
        (local_half.x * scale.x).max(0.01),
        (local_half.y * scale.y).max(0.01),
        (local_half.z * scale.z).max(0.01),
    ))
}

fn update_dropped_world_item_physics_from_decoded(
    world: &mut newengine_ecs::World,
    owner: EntityId,
    decoded: &[DecodedPrefabMeshPart],
) -> Result<Vec3, String> {
    let runtime = world
        .get::<newengine_engine_runtime::gameplay::WorldItemRuntime>(owner)
        .copied()
        .ok_or_else(|| "world item has no runtime component".to_owned())?;
    if !runtime.dropped {
        return Ok(Vec3::ZERO);
    }
    let presentation = world
        .get::<newengine_engine_runtime::gameplay::WorldItemPresentation>(owner)
        .cloned()
        .ok_or_else(|| "dropped world item has no presentation".to_owned())?;
    let (min, max) = decoded_model_bounds(decoded)?;
    let half_extents = scaled_world_item_half_extents(min, max, presentation.scale)?;
    let mut body = newengine_engine_runtime::gameplay::PhysicsBodyDesc::dynamic_solid(
        newengine_engine_runtime::gameplay::CollisionShapeDesc::Box {
            half_extents: [half_extents.x, half_extents.y, half_extents.z],
        },
    );
    if let Some(existing) = world
        .get::<newengine_engine_runtime::gameplay::PhysicsBodyDesc>(owner)
        .copied()
    {
        body.material = existing.material;
    }
    let _ = world.insert(owner, body);
    let _ = world.insert(
        owner,
        Bounds::from_local_aabb(newengine_bounds::Aabb::from_center_half_extents(
            Vec3::ZERO,
            half_extents,
        )),
    );
    Ok(half_extents)
}

fn try_admit_runtime_world_item_visual(
    world: &mut newengine_ecs::World,
    prims: &mut PrimitiveRegistry,
    mats: &MaterialRegistry,
    owner: EntityId,
) -> Result<(), String> {
    let presentation = world
        .get::<newengine_engine_runtime::gameplay::WorldItemPresentation>(owner)
        .cloned()
        .ok_or_else(|| "runtime world item has no presentation".to_owned())?;
    let model_ref = presentation
        .model_ref
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "runtime world item has no authored model_ref".to_owned())?;
    let pickup = world
        .get::<newengine_engine_runtime::gameplay::ItemPickup>(owner)
        .copied()
        .ok_or_else(|| "runtime world item has no ItemPickup".to_owned())?;
    let definition = world
        .resource::<newengine_engine_runtime::gameplay::ItemCatalog>()
        .and_then(|catalog| catalog.get(pickup.item))
        .cloned()
        .ok_or_else(|| "runtime world item definition is unavailable".to_owned())?;
    let world_definition = definition.world.clone().sanitized();
    let decoded = decode_runtime_ydd_prefab(model_ref).map_err(|error| {
        format!("runtime world-item model decode failed path='{model_ref}': {error}")
    })?;
    let runtime = world
        .get::<newengine_engine_runtime::gameplay::WorldItemRuntime>(owner)
        .copied()
        .ok_or_else(|| "runtime world item has no WorldItemRuntime".to_owned())?;
    let pickup_id = if runtime.dropped {
        format!("drop-{:016x}", runtime.persistent_id)
    } else {
        format!("runtime-{:016x}", runtime.persistent_id)
    };
    let parts = bind_world_item_model_from_decoded(
        world,
        prims,
        mats,
        owner,
        &pickup_id,
        Vec3::ONE,
        &decoded,
        world_definition.material_library_ref.as_deref(),
    )?;
    let physical_half_extents =
        update_dropped_world_item_physics_from_decoded(world, owner, &decoded)?;
    let slots = decoded
        .iter()
        .map(|part| part.material_slot.as_str())
        .collect::<Vec<_>>();
    let resolved = decoded
        .iter()
        .map(|part| {
            world_item_material_asset(
                part.material_ref.as_deref(),
                &part.material_slot,
                world_definition.material_library_ref.as_deref(),
            )
            .unwrap_or_default()
        })
        .collect::<Vec<_>>();
    newengine_ulog_api::ulog::info!(
        "WORLD_ITEM_VISUAL owner={} item='{}' model='{}' parts={} slots={:?} resolved_materials={:?} textures_required='base+normal+roughness' fallback_used=false",
        owner.stable_u64(),
        definition.name,
        model_ref,
        parts,
        slots,
        resolved,
    );
    if runtime.dropped {
        newengine_ulog_api::ulog::info!(
            "WORLD_ITEM_PHYSICS owner={} item='{}' body_created=true collider_created=true shape='oriented_box_from_ydd_bounds' half_extents={:?} interaction_half_extents={:?} dynamic=true pickup_trigger_separate=true",
            owner.stable_u64(),
            definition.name,
            physical_half_extents,
            presentation.pickup_half_extents,
        );
    }
    Ok(())
}

pub(super) fn tick_runtime_world_item_visuals(
    world: &mut newengine_ecs::World,
    prims: &mut PrimitiveRegistry,
    mats: &MaterialRegistry,
) {
    // Model-backed inventory items use an invisible staging root until their authored
    // YDD/NEMAT/YTD hierarchy is fully ready. Admission state is explicit; it must not be
    // inferred from a visible fallback Primitive, because that is exactly how white placeholder
    // objects leaked into production rendering.
    let candidates = world
        .query::<newengine_engine_runtime::gameplay::WorldItemPresentation>()
        .filter_map(|(owner, presentation)| {
            let authored_model = presentation
                .model_ref
                .as_deref()
                .is_some_and(|value| !value.trim().is_empty());
            (authored_model && !presentation.authored_visual_admitted).then_some(owner)
        })
        .collect::<Vec<_>>();

    for owner in candidates {
        match try_admit_runtime_world_item_visual(world, prims, mats, owner) {
            Ok(()) => {
                if let Some(presentation) =
                    world
                        .get_mut::<newengine_engine_runtime::gameplay::WorldItemPresentation>(owner)
                {
                    presentation.authored_visual_admitted = true;
                }
                let _ = world.remove::<RuntimeWorldItemAdmissionState>(owner);
            }
            Err(error) => {
                let attempts = world
                    .get::<RuntimeWorldItemAdmissionState>(owner)
                    .copied()
                    .unwrap_or_default()
                    .attempts
                    .saturating_add(1);
                let _ = world.insert(owner, RuntimeWorldItemAdmissionState { attempts });
                if attempts == 1 || attempts.is_multiple_of(60) {
                    newengine_ulog_api::ulog::warn!(
                        "game-ready runtime world-item admission deferred owner={} attempt={} err='{}' policy='authored model never persists as fallback primitive'",
                        owner.stable_u64(),
                        attempts,
                        error,
                    );
                }
            }
        }
    }
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
) -> Result<GameReadyMissionSpawnSummary, String> {
    let mut summary = GameReadyMissionSpawnSummary::default();
    for material_ref in [
        mission.core_material.as_deref(),
        mission.target_material.as_deref(),
        mission.hazard_material.as_deref(),
        mission.goal_material.as_deref(),
    ]
    .into_iter()
    .flatten()
    {
        if let Err(error) = pin_mission_asset(world, material_ref) {
            newengine_ulog_api::ulog::warn!(
                "game-ready mission asset pin failed asset='{}' class='mission' owner='{}' err='{}'",
                material_ref,
                MISSION_STREAMING_PIN_OWNER,
                error,
            );
        }
    }
    let materials = register_mission_materials(mats, mission)?;

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
            materials.core.expect("mission material validated"),
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
            materials.target.expect("mission material validated"),
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
        let _ = world.insert(
            entity,
            newengine_engine_runtime::gameplay::DamageReceiver::character(),
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
            materials.hazard.expect("mission material validated"),
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
            materials.goal.expect("mission material validated"),
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
        "game-ready mission spawned: pickups={} item_pickups={} targets={} hazards={} goals={} policy='all generic mission presentation materials are project-authored'",
        summary.pickups,
        summary.item_pickups,
        summary.targets,
        summary.hazards,
        summary.goals,
    );
    Ok(summary)
}

#[cfg(test)]
mod world_item_runtime_tests {
    use super::*;

    #[test]
    fn world_item_material_library_is_scoped_by_mesh_slot() {
        assert_eq!(
            world_item_material_asset(Some("shared/materials/weapon_rifle.nemat"), "m00", None,)
                .as_deref(),
            Some("shared/materials/weapon_rifle.nemat@m00")
        );
        assert_eq!(
            world_item_material_asset(
                Some("shared/materials/weapon_rifle.nemat@m01"),
                "m00",
                None,
            )
            .as_deref(),
            Some("shared/materials/weapon_rifle.nemat@m01")
        );
        assert_eq!(
            world_item_material_asset(None, "m01", Some("shared/materials/weapon_rifle.nemat"))
                .as_deref(),
            Some("shared/materials/weapon_rifle.nemat@m01")
        );
    }

    #[test]
    fn dropped_rifle_physics_uses_canonical_ydd_bounds_not_pickup_box() {
        let min = Vec3::new(-0.069_917_45, -0.065_805_55, -0.372_692_38);
        let max = Vec3::new(0.120_714_34, 0.127_575_71, 0.633_752_35);
        let half = scaled_world_item_half_extents(min, max, Vec3::ONE).expect("rifle bounds");
        assert!((half.x - 0.095_315_9).abs() < 1.0e-5, "half={half:?}");
        assert!((half.y - 0.096_690_63).abs() < 1.0e-5, "half={half:?}");
        assert!((half.z - 0.503_222_35).abs() < 1.0e-5, "half={half:?}");
        assert!(
            half.z > half.x * 5.0,
            "rifle collider must stay elongated: {half:?}"
        );
    }

    #[test]
    fn authored_world_item_render_path_casts_and_receives_shadows() {
        let options = world_item_render_options();
        assert_eq!(
            options.shadow_policy,
            newengine_model_domain_api::MeshShadowPolicy::CastAndReceive
        );
    }
}
