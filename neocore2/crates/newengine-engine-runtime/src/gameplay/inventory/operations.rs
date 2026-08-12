use super::*;

/// Ensures only generic inventory runtime resources.
///
/// Item definitions and loadouts are profile-owned content and must be installed explicitly
/// through a gameplay content provider. This function deliberately does not create catalogs.
pub fn ensure_inventory_runtime(world: &mut World) {
    if world.resource::<InventoryEventBus>().is_none() {
        world.insert_resource(InventoryEventBus::default());
    }
}

pub fn ensure_player_inventory(world: &mut World, owner: EntityId) {
    ensure_inventory_runtime(world);
    if world.get::<PlayerInventory>(owner).is_none() {
        let _ = world.insert(owner, PlayerInventory::default());
    }
}

pub fn give_item(
    world: &mut World,
    owner: EntityId,
    item: ItemId,
    quantity: u32,
) -> Result<InventoryMutation, String> {
    ensure_player_inventory(world, owner);
    let catalog = world.resource::<ItemCatalog>().cloned().ok_or_else(|| {
        "item catalog is unavailable; install a gameplay content provider".to_owned()
    })?;
    let definition = catalog
        .get(item)
        .cloned()
        .ok_or_else(|| format!("unknown item {:016x}", item.0))?;
    let mutation = world
        .get_mut::<PlayerInventory>(owner)
        .ok_or_else(|| "owner has no inventory".to_owned())?
        .add_definition(owner, &definition, quantity, &catalog);

    if mutation.accepted > 0 {
        emit_inventory_event(
            world,
            InventoryEvent {
                kind: InventoryEventKind::ItemAdded,
                owner,
                item,
                instance_id: mutation.touched_instances.last().copied(),
                quantity: mutation.accepted,
                slot: definition.equipment_slot,
                world_entity: None,
                message: format!("added {} x{}", definition.name, mutation.accepted),
            },
        );
    }
    Ok(mutation)
}

pub fn remove_item(
    world: &mut World,
    owner: EntityId,
    item: ItemId,
    quantity: u32,
) -> Result<InventoryMutation, String> {
    let definition_name = world
        .resource::<ItemCatalog>()
        .and_then(|catalog| catalog.get(item))
        .map(|definition| definition.name.clone())
        .unwrap_or_else(|| format!("{:016x}", item.0));
    let mutation = world
        .get_mut::<PlayerInventory>(owner)
        .ok_or_else(|| "owner has no inventory".to_owned())?
        .remove_quantity(item, quantity);
    if mutation.accepted > 0 {
        emit_inventory_event(
            world,
            InventoryEvent {
                kind: InventoryEventKind::ItemRemoved,
                owner,
                item,
                instance_id: mutation.touched_instances.last().copied(),
                quantity: mutation.accepted,
                slot: None,
                world_entity: None,
                message: format!("removed {} x{}", definition_name, mutation.accepted),
            },
        );
        sync_equipped_weapon_runtime(world, owner);
    }
    Ok(mutation)
}

#[inline]
pub fn inventory_quantity(world: &World, owner: EntityId, item: ItemId) -> u32 {
    world
        .get::<PlayerInventory>(owner)
        .map(|inventory| inventory.quantity(item))
        .unwrap_or(0)
}

pub fn apply_loadout(world: &mut World, owner: EntityId, loadout: ItemId) -> Result<(), String> {
    ensure_player_inventory(world, owner);
    let loadout = world
        .resource::<InventoryLoadoutCatalog>()
        .and_then(|catalog| catalog.get(loadout))
        .cloned()
        .ok_or_else(|| {
            "loadout definition is unavailable; install a gameplay content provider".to_owned()
        })?;
    if loadout.clear_existing {
        persist_equipped_weapon_state(world, owner);
        if let Some(inventory) = world.get_mut::<PlayerInventory>(owner) {
            inventory.entries.clear();
            inventory.equipped.clear();
            inventory.weapon_states.clear();
            inventory.active_slot = None;
        }
        let _ = world.remove::<EquippedWeaponBinding>(owner);
        let _ = world.remove::<HitscanWeaponTuning>(owner);
        let _ = world.remove::<PlayerWeaponState>(owner);
    }
    for entry in &loadout.entries {
        let mutation = give_item(world, owner, entry.item, entry.quantity)?;
        if mutation.accepted == 0 {
            continue;
        }
        if entry.equip {
            equip_first_item(world, owner, entry.item)?;
        }
    }
    if let Some(inventory) = world.get_mut::<PlayerInventory>(owner) {
        inventory.mark_loadout_initialized();
    }
    emit_inventory_event(
        world,
        InventoryEvent {
            kind: InventoryEventKind::LoadoutApplied,
            owner,
            item: loadout.id,
            instance_id: None,
            quantity: loadout.entries.len().min(u32::MAX as usize) as u32,
            slot: None,
            world_entity: None,
            message: format!("applied {}", loadout.name),
        },
    );
    sync_equipped_weapon_runtime(world, owner);
    Ok(())
}

pub fn drain_inventory_events(world: &mut World) -> Vec<InventoryEvent> {
    world
        .resource_mut::<InventoryEventBus>()
        .map(InventoryEventBus::drain)
        .unwrap_or_default()
}

pub(super) fn emit_inventory_event(world: &mut World, event: InventoryEvent) {
    ensure_inventory_runtime(world);
    if let Some(bus) = world.resource_mut::<InventoryEventBus>() {
        bus.emit(event);
    }
}
