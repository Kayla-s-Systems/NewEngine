use super::operations::emit_inventory_event;
use super::*;

pub fn spawn_item_pickup(
    world: &mut World,
    root: Option<EntityId>,
    item: ItemId,
    quantity: u32,
    position: Vec3,
) -> Result<EntityId, String> {
    spawn_item_pickup_internal(world, root, item, quantity, position, None, None)
}

pub fn spawn_persistent_item_pickup(
    world: &mut World,
    root: Option<EntityId>,
    item: ItemId,
    quantity: u32,
    position: Vec3,
    persistent_key: &str,
    respawn_seconds: f32,
) -> Result<EntityId, String> {
    let persistent_key = persistent_key.trim();
    if persistent_key.is_empty() {
        return Err("persistent pickup key must not be empty".to_owned());
    }
    spawn_item_pickup_internal(
        world,
        root,
        item,
        quantity,
        position,
        Some(stable_hash64(persistent_key.as_bytes())),
        Some(respawn_seconds),
    )
}

fn spawn_item_pickup_internal(
    world: &mut World,
    root: Option<EntityId>,
    item: ItemId,
    quantity: u32,
    position: Vec3,
    persistent_id: Option<u64>,
    respawn_seconds: Option<f32>,
) -> Result<EntityId, String> {
    let definition = world
        .resource::<ItemCatalog>()
        .and_then(|catalog| catalog.get(item))
        .cloned()
        .ok_or_else(|| "item definition is unavailable".to_owned())?;
    let world_definition = definition.world.clone().sanitized();
    let half_extents = Vec3::new(
        world_definition.pickup_half_extents[0],
        world_definition.pickup_half_extents[1],
        world_definition.pickup_half_extents[2],
    );
    let scale = Vec3::new(
        world_definition.scale[0],
        world_definition.scale[1],
        world_definition.scale[2],
    );
    let entity = world.spawn();
    let quantity = quantity.max(1);
    let _ = world.insert(entity, Name(format!("pickup:{}", definition.name)));
    let _ = world.insert(entity, GameplayActor);
    attach_scene_object_core(world, entity, position, half_extents);
    let mut pickup = ItemPickup::new(item, quantity);
    let effective_respawn = respawn_seconds
        .unwrap_or(world_definition.respawn_seconds)
        .max(0.0);
    // Authored/source pickups remain as dormant entities after collection so editor
    // Play/Stop snapshots can restore them without fabricating EntityId values.
    pickup.destroy_when_empty = false;
    let _ = world.insert(entity, pickup);
    let _ = world.insert(
        entity,
        Interactable::new(format!("Pick up {}", definition.display_name)),
    );
    let _ = world.insert(entity, PhysicsSurface::default());
    let visual_entity = world.spawn();
    let _ = world.insert(
        visual_entity,
        Name(format!("pickup-visual:{}", definition.name)),
    );
    let _ = world.insert(
        visual_entity,
        Transform {
            scale,
            ..Transform::default()
        },
    );
    let _ = world.insert(
        visual_entity,
        Primitive {
            id: world_definition.fallback_primitive,
            color: world_definition.color,
        },
    );
    let _ = world.insert(visual_entity, DisplayVisibility::default());
    let _ = world.insert(visual_entity, WorldItemVisualPart { owner: entity });
    let _ = set_parent(world, visual_entity, Some(entity));
    let _ = world.insert(
        entity,
        WorldItemPresentation {
            visual_entity,
            model_ref: world_definition.model_ref,
            fallback_primitive: world_definition.fallback_primitive,
            scale,
            color: world_definition.color,
            pickup_half_extents: half_extents,
        },
    );
    let stable_id = persistent_id
        .unwrap_or_else(|| mix64(entity.stable_u64() ^ item.0.rotate_left(13) ^ quantity as u64));
    let _ = world.insert(
        entity,
        WorldItemRuntime::persistent_source(stable_id, position, quantity, effective_respawn),
    );
    let _ = world.insert(entity, DisplayVisibility::default());
    if let Some(root) = root.filter(|candidate| world.exists(*candidate)) {
        let _ = set_parent(world, entity, Some(root));
    }
    Ok(entity)
}

pub fn drop_item(
    world: &mut World,
    owner: EntityId,
    item: ItemId,
    quantity: u32,
) -> Result<EntityId, String> {
    let transform = world
        .get::<Transform>(owner)
        .copied()
        .ok_or_else(|| "owner has no transform".to_owned())?;
    let removed = world
        .get_mut::<PlayerInventory>(owner)
        .ok_or_else(|| "owner has no inventory".to_owned())?
        .remove_quantity(item, quantity);
    if removed.accepted == 0 {
        return Err("item is not present in inventory".to_owned());
    }
    let forward = (transform.rotation * Vec3::new(0.0, 0.0, -1.0)).normalize_or_zero();
    let spawn_position = transform.position + forward * 0.9 + Vec3::Y * 0.6;
    let entity = spawn_item_pickup(world, None, item, removed.accepted, spawn_position)?;
    let presentation = world
        .get::<WorldItemPresentation>(entity)
        .cloned()
        .ok_or_else(|| "dropped item has no world presentation".to_owned())?;
    let _ = world.insert(
        entity,
        PhysicsBodyDesc::dynamic_solid(CollisionShapeDesc::Box {
            half_extents: [
                presentation.pickup_half_extents.x,
                presentation.pickup_half_extents.y,
                presentation.pickup_half_extents.z,
            ],
        }),
    );
    let _ = world.insert(entity, Velocity(forward * 2.4 + Vec3::Y * 1.6));
    let _ = world.insert(entity, AngularVelocity(Vec3::new(1.8, 3.2, 0.9)));
    let persistent_id = mix64(entity.stable_u64() ^ item.0.rotate_left(31));
    let _ = world.insert(
        entity,
        WorldItemRuntime::dropped(persistent_id, spawn_position, removed.accepted),
    );
    if let Some(pickup) = world.get_mut::<ItemPickup>(entity) {
        pickup.enabled = false;
        pickup.destroy_when_empty = true;
    }
    if let Some(interactable) = world.get_mut::<Interactable>(entity) {
        interactable.enabled = false;
    }
    emit_inventory_event(
        world,
        InventoryEvent {
            kind: InventoryEventKind::ItemDropped,
            owner,
            item,
            instance_id: removed.touched_instances.last().copied(),
            quantity: removed.accepted,
            slot: None,
            world_entity: Some(entity),
            message: "item dropped into world".to_owned(),
        },
    );
    sync_equipped_weapon_runtime(world, owner);
    Ok(entity)
}

pub fn step_world_items(world: &mut World, dt: f32) {
    let dt = if dt.is_finite() && dt > 0.0 {
        dt.min(0.25)
    } else {
        0.0
    };
    let entities = world
        .query::<WorldItemRuntime>()
        .map(|(entity, runtime)| (entity, *runtime))
        .collect::<Vec<_>>();

    for (entity, mut runtime) in entities {
        let mut changed = false;
        if runtime.pickup_cooldown_remaining > 0.0 {
            runtime.pickup_cooldown_remaining = (runtime.pickup_cooldown_remaining - dt).max(0.0);
            changed = true;
            if runtime.pickup_cooldown_remaining == 0.0 && runtime.respawn_remaining <= 0.0 {
                if let Some(pickup) = world.get_mut::<ItemPickup>(entity) {
                    pickup.enabled = true;
                }
                if let Some(interactable) = world.get_mut::<Interactable>(entity) {
                    interactable.enabled = true;
                }
            }
        }

        if runtime.respawn_remaining > 0.0 {
            runtime.respawn_remaining = (runtime.respawn_remaining - dt).max(0.0);
            changed = true;
            if runtime.respawn_remaining == 0.0 {
                restore_respawned_world_item(world, entity, runtime);
            }
        }

        if changed && world.exists(entity) {
            let _ = world.insert(entity, runtime);
        }
    }
}

fn restore_respawned_world_item(world: &mut World, entity: EntityId, runtime: WorldItemRuntime) {
    let Some(presentation) = world.get::<WorldItemPresentation>(entity).cloned() else {
        return;
    };
    if let Some(transform) = world.get_mut::<Transform>(entity) {
        transform.position = runtime.spawn_position;
        transform.scale = Vec3::ONE;
    }
    if let Some(transform) = world.get_mut::<Transform>(presentation.visual_entity) {
        transform.position = Vec3::ZERO;
        transform.scale = presentation.scale;
    }
    if let Some(pickup) = world.get_mut::<ItemPickup>(entity) {
        pickup.quantity = runtime.original_quantity;
        pickup.enabled = true;
    }
    if let Some(interactable) = world.get_mut::<Interactable>(entity) {
        interactable.enabled = true;
    }
    let body = PhysicsBodyDesc::trigger(CollisionShapeDesc::Box {
        half_extents: [
            presentation.pickup_half_extents.x,
            presentation.pickup_half_extents.y,
            presentation.pickup_half_extents.z,
        ],
    });
    let _ = world.insert(entity, body);
    let _ = world.insert(entity, body.to_bounds());
    set_world_item_visibility(world, entity, DisplayMode::Both);
    let _ = world.remove::<Velocity>(entity);
    let _ = world.remove::<AngularVelocity>(entity);
}

fn set_world_item_visibility(world: &mut World, entity: EntityId, mode: DisplayMode) {
    let visual_entity = world
        .get::<WorldItemPresentation>(entity)
        .map(|presentation| presentation.visual_entity);
    let _ = world.insert(entity, DisplayVisibility { mode });
    if let Some(visual_entity) = visual_entity.filter(|visual| world.exists(*visual)) {
        let _ = world.insert(visual_entity, DisplayVisibility { mode });
    }
}

fn despawn_world_item(world: &mut World, entity: EntityId) {
    let visual_entity = world
        .get::<WorldItemPresentation>(entity)
        .map(|presentation| presentation.visual_entity);
    if let Some(visual_entity) = visual_entity.filter(|visual| world.exists(*visual)) {
        let _ = world.despawn(visual_entity);
    }
    let _ = world.despawn(entity);
}

fn deactivate_consumed_world_item(world: &mut World, entity: EntityId) {
    if let Some(pickup) = world.get_mut::<ItemPickup>(entity) {
        pickup.enabled = false;
        pickup.quantity = 0;
    }
    if let Some(interactable) = world.get_mut::<Interactable>(entity) {
        interactable.enabled = false;
    }
    set_world_item_visibility(world, entity, DisplayMode::RuntimeHidden);
    let _ = world.remove::<PhysicsBodyDesc>(entity);
    let _ = world.remove::<Velocity>(entity);
    let _ = world.remove::<AngularVelocity>(entity);
}

fn deactivate_world_item_for_respawn(
    world: &mut World,
    entity: EntityId,
    runtime: &mut WorldItemRuntime,
) {
    runtime.respawn_remaining = runtime.respawn_seconds.max(0.001);
    runtime.pickup_cooldown_remaining = 0.0;
    if let Some(pickup) = world.get_mut::<ItemPickup>(entity) {
        pickup.enabled = false;
        pickup.quantity = 0;
    }
    if let Some(interactable) = world.get_mut::<Interactable>(entity) {
        interactable.enabled = false;
    }
    set_world_item_visibility(world, entity, DisplayMode::RuntimeHidden);
    let _ = world.remove::<PhysicsBodyDesc>(entity);
    let _ = world.remove::<Velocity>(entity);
    let _ = world.remove::<AngularVelocity>(entity);
    let _ = world.insert(entity, *runtime);
}

pub fn try_collect_item_pickup(
    world: &mut World,
    owner: EntityId,
    pickup_entity: EntityId,
) -> bool {
    let Some(mut pickup) = world.get::<ItemPickup>(pickup_entity).copied() else {
        return false;
    };
    if !pickup.enabled || pickup.quantity == 0 {
        return false;
    }
    let mutation = match give_item(world, owner, pickup.item, pickup.quantity) {
        Ok(mutation) => mutation,
        Err(error) => {
            emit_inventory_event(
                world,
                InventoryEvent {
                    kind: InventoryEventKind::PickupRejected,
                    owner,
                    item: pickup.item,
                    instance_id: None,
                    quantity: pickup.quantity,
                    slot: None,
                    world_entity: Some(pickup_entity),
                    message: error,
                },
            );
            return false;
        }
    };
    if mutation.accepted == 0 {
        emit_inventory_event(
            world,
            InventoryEvent {
                kind: InventoryEventKind::PickupRejected,
                owner,
                item: pickup.item,
                instance_id: None,
                quantity: pickup.quantity,
                slot: None,
                world_entity: Some(pickup_entity),
                message: "inventory capacity or weight limit reached".to_owned(),
            },
        );
        return false;
    }

    pickup.quantity -= mutation.accepted;
    if pickup.auto_equip {
        let _ = equip_first_item(world, owner, pickup.item);
    }
    emit_inventory_event(
        world,
        InventoryEvent {
            kind: InventoryEventKind::PickupCollected,
            owner,
            item: pickup.item,
            instance_id: mutation.touched_instances.last().copied(),
            quantity: mutation.accepted,
            slot: None,
            world_entity: Some(pickup_entity),
            message: "world pickup transferred into inventory".to_owned(),
        },
    );

    if pickup.quantity == 0 {
        pickup.enabled = false;
        let mut runtime = world.get::<WorldItemRuntime>(pickup_entity).copied();
        if let Some(runtime) = runtime
            .as_mut()
            .filter(|runtime| runtime.respawn_seconds > 0.0)
        {
            let _ = world.insert(pickup_entity, pickup);
            deactivate_world_item_for_respawn(world, pickup_entity, runtime);
        } else if runtime.is_some_and(|runtime| !runtime.dropped) {
            let _ = world.insert(pickup_entity, pickup);
            deactivate_consumed_world_item(world, pickup_entity);
        } else if pickup.destroy_when_empty {
            despawn_world_item(world, pickup_entity);
        } else {
            let _ = world.insert(pickup_entity, pickup);
            deactivate_consumed_world_item(world, pickup_entity);
        }
    } else {
        let _ = world.insert(pickup_entity, pickup);
    }
    true
}
