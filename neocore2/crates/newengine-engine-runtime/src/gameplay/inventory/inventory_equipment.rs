use super::operations::emit_inventory_event;
use super::*;

pub fn equip_first_item(world: &mut World, owner: EntityId, item: ItemId) -> Result<(), String> {
    let instance = world
        .get::<PlayerInventory>(owner)
        .and_then(|inventory| {
            inventory
                .entries
                .iter()
                .find(|entry| entry.item == item && entry.quantity > 0)
                .map(|entry| entry.instance_id)
        })
        .ok_or_else(|| "item is not present in inventory".to_owned())?;
    equip_item_instance(world, owner, instance)
}

pub fn equip_item_instance(
    world: &mut World,
    owner: EntityId,
    instance: ItemInstanceId,
) -> Result<(), String> {
    ensure_player_inventory(world, owner);
    persist_equipped_weapon_state(world, owner);
    let catalog = world
        .resource::<ItemCatalog>()
        .cloned()
        .ok_or_else(|| "item catalog is unavailable".to_owned())?;
    let item = world
        .get::<PlayerInventory>(owner)
        .and_then(|inventory| inventory.entry(instance))
        .map(|entry| entry.item)
        .ok_or_else(|| "item instance is not present in inventory".to_owned())?;
    let definition = catalog
        .get(item)
        .cloned()
        .ok_or_else(|| "item definition is unavailable".to_owned())?;
    let slot = definition
        .equipment_slot
        .ok_or_else(|| "item cannot be equipped".to_owned())?;

    let (previous, previous_item) = {
        let inventory = world
            .get_mut::<PlayerInventory>(owner)
            .ok_or_else(|| "owner has no inventory".to_owned())?;
        let previous = inventory.equipped.get(&slot).copied();
        let previous_item = previous
            .and_then(|previous| inventory.entry(previous))
            .map(|entry| entry.item);
        inventory.equipped.insert(slot, instance);
        if definition.kind == ItemKind::Weapon {
            inventory.active_slot = Some(slot);
        }
        (previous, previous_item)
    };

    if let Some(previous) = previous.filter(|previous| *previous != instance) {
        emit_inventory_event(
            world,
            InventoryEvent {
                kind: InventoryEventKind::Unequipped,
                owner,
                item: previous_item.unwrap_or(item),
                instance_id: Some(previous),
                quantity: 1,
                slot: Some(slot),
                world_entity: None,
                message: "slot occupant replaced".to_owned(),
            },
        );
    }
    emit_inventory_event(
        world,
        InventoryEvent {
            kind: InventoryEventKind::Equipped,
            owner,
            item,
            instance_id: Some(instance),
            quantity: 1,
            slot: Some(slot),
            world_entity: None,
            message: format!("equipped {}", definition.name),
        },
    );
    sync_equipped_weapon_runtime(world, owner);
    Ok(())
}

pub fn select_equipment_slot(
    world: &mut World,
    owner: EntityId,
    slot: EquipmentSlot,
) -> Result<(), String> {
    persist_equipped_weapon_state(world, owner);
    let instance = world
        .get::<PlayerInventory>(owner)
        .and_then(|inventory| inventory.equipped_instance(slot))
        .ok_or_else(|| "equipment slot is empty".to_owned())?;
    let item = world
        .get::<PlayerInventory>(owner)
        .and_then(|inventory| inventory.entry(instance))
        .map(|entry| entry.item)
        .ok_or_else(|| "equipped item instance is missing".to_owned())?;
    let matches_slot = world
        .resource::<ItemCatalog>()
        .and_then(|catalog| catalog.get(item))
        .map(|definition| definition.equipment_slot == Some(slot))
        .unwrap_or(false);
    if !matches_slot {
        return Err("selected equipment slot contains an incompatible item".to_owned());
    }
    if let Some(inventory) = world.get_mut::<PlayerInventory>(owner) {
        inventory.active_slot = Some(slot);
    }
    emit_inventory_event(
        world,
        InventoryEvent {
            kind: InventoryEventKind::ActiveSlotChanged,
            owner,
            item,
            instance_id: Some(instance),
            quantity: 1,
            slot: Some(slot),
            world_entity: None,
            message: "active equipment slot changed".to_owned(),
        },
    );
    sync_equipped_weapon_runtime(world, owner);
    Ok(())
}

pub fn unequip_slot(world: &mut World, owner: EntityId, slot: EquipmentSlot) -> Result<(), String> {
    persist_equipped_weapon_state(world, owner);
    let removed = world
        .get_mut::<PlayerInventory>(owner)
        .and_then(|inventory| {
            let removed = inventory.equipped.remove(&slot);
            if inventory.active_slot == Some(slot) {
                inventory.active_slot = None;
            }
            removed
        })
        .ok_or_else(|| "equipment slot is empty".to_owned())?;
    let item = world
        .get::<PlayerInventory>(owner)
        .and_then(|inventory| inventory.entry(removed))
        .map(|entry| entry.item)
        .unwrap_or_default();
    emit_inventory_event(
        world,
        InventoryEvent {
            kind: InventoryEventKind::Unequipped,
            owner,
            item,
            instance_id: Some(removed),
            quantity: 1,
            slot: Some(slot),
            world_entity: None,
            message: "equipment slot cleared".to_owned(),
        },
    );
    sync_equipped_weapon_runtime(world, owner);
    Ok(())
}

pub fn sync_equipped_weapon_runtime(world: &mut World, owner: EntityId) {
    let selected = selected_weapon(world, owner);
    let current_binding = world.get::<EquippedWeaponBinding>(owner).copied();
    let current_state = world.get::<PlayerWeaponState>(owner).copied();

    if current_binding.map(|binding| binding.instance_id)
        != selected
            .as_ref()
            .map(|selected| selected.binding.instance_id)
    {
        if let (Some(binding), Some(state)) = (current_binding, current_state) {
            if let Some(inventory) = world.get_mut::<PlayerInventory>(owner) {
                if inventory.entry(binding.instance_id).is_some() {
                    inventory.weapon_states.insert(binding.instance_id, state);
                }
            }
        }
    }

    let Some(selected) = selected else {
        let _ = world.remove::<EquippedWeaponBinding>(owner);
        let _ = world.remove::<HitscanWeaponTuning>(owner);
        let _ = world.remove::<PlayerWeaponState>(owner);
        return;
    };

    let reserve = inventory_quantity(world, owner, selected.binding.ammo_item);
    if current_binding == Some(selected.binding) {
        if world.get::<HitscanWeaponTuning>(owner).is_none() {
            let _ = world.insert(owner, selected.tuning);
        }
        if let Some(state) = world.get_mut::<PlayerWeaponState>(owner) {
            state.reserve_ammo = reserve;
        } else {
            let mut state = selected.stored_state;
            state.reserve_ammo = reserve;
            let _ = world.insert(owner, state);
        }
        return;
    }

    let mut state = selected.stored_state;
    state.reserve_ammo = reserve;
    let _ = world.insert(owner, selected.binding);
    let _ = world.insert(owner, selected.tuning);
    let _ = world.insert(owner, state);
}

pub fn persist_equipped_weapon_state(world: &mut World, owner: EntityId) {
    let Some(binding) = world.get::<EquippedWeaponBinding>(owner).copied() else {
        return;
    };
    let Some(state) = world.get::<PlayerWeaponState>(owner).copied() else {
        return;
    };
    if let Some(inventory) = world.get_mut::<PlayerInventory>(owner) {
        if inventory.entry(binding.instance_id).is_some() {
            inventory.weapon_states.insert(binding.instance_id, state);
        }
    }
}

pub fn equipped_reserve_ammo(world: &World, owner: EntityId) -> Option<u32> {
    let binding = world.get::<EquippedWeaponBinding>(owner)?;
    Some(inventory_quantity(world, owner, binding.ammo_item))
}

pub fn consume_equipped_ammo(world: &mut World, owner: EntityId, requested: u32) -> u32 {
    let Some(binding) = world.get::<EquippedWeaponBinding>(owner).copied() else {
        return 0;
    };
    let mutation = world
        .get_mut::<PlayerInventory>(owner)
        .map(|inventory| inventory.remove_quantity(binding.ammo_item, requested))
        .unwrap_or_default();
    if mutation.accepted > 0 {
        emit_inventory_event(
            world,
            InventoryEvent {
                kind: InventoryEventKind::AmmoConsumed,
                owner,
                item: binding.ammo_item,
                instance_id: mutation.touched_instances.last().copied(),
                quantity: mutation.accepted,
                slot: Some(binding.slot),
                world_entity: None,
                message: "ammunition transferred into magazine".to_owned(),
            },
        );
    }
    mutation.accepted
}

pub fn use_item(world: &mut World, owner: EntityId, item: ItemId) -> Result<(), String> {
    let definition = world
        .resource::<ItemCatalog>()
        .and_then(|catalog| catalog.get(item))
        .cloned()
        .ok_or_else(|| "item definition is unavailable".to_owned())?;
    if inventory_quantity(world, owner, item) == 0 {
        return Err("item is not present in inventory".to_owned());
    }

    match definition.use_effect {
        ItemUseEffect::None => return Err("item has no usable effect".to_owned()),
        ItemUseEffect::Heal { amount } => {
            let health = world
                .get_mut::<Health>(owner)
                .ok_or_else(|| "owner has no health component".to_owned())?;
            if health.current >= health.maximum || amount <= 0.0 {
                return Err("healing item would have no effect".to_owned());
            }
            health.current = (health.current + amount).min(health.maximum);
        }
    }

    let removed = world
        .get_mut::<PlayerInventory>(owner)
        .map(|inventory| inventory.remove_quantity(item, 1))
        .unwrap_or_default();
    if removed.accepted != 1 {
        return Err("failed to consume inventory item".to_owned());
    }
    emit_inventory_event(
        world,
        InventoryEvent {
            kind: InventoryEventKind::ItemUsed,
            owner,
            item,
            instance_id: removed.touched_instances.last().copied(),
            quantity: 1,
            slot: definition.equipment_slot,
            world_entity: None,
            message: format!("used {}", definition.name),
        },
    );
    Ok(())
}

#[derive(Clone, Copy)]
struct SelectedWeapon {
    binding: EquippedWeaponBinding,
    tuning: HitscanWeaponTuning,
    stored_state: PlayerWeaponState,
}

fn selected_weapon(world: &World, owner: EntityId) -> Option<SelectedWeapon> {
    let inventory = world.get::<PlayerInventory>(owner)?;
    let slot = inventory.active_slot?;
    let instance_id = inventory.equipped_instance(slot)?;
    let entry = inventory.entry(instance_id)?;
    let definition = world.resource::<ItemCatalog>()?.get(entry.item)?;
    let weapon = definition.weapon?;
    let mut state = inventory
        .weapon_states
        .get(&instance_id)
        .copied()
        .unwrap_or_else(|| PlayerWeaponState::loaded(weapon.tuning));
    state.reserve_ammo = inventory.quantity(weapon.ammo_item);
    Some(SelectedWeapon {
        binding: EquippedWeaponBinding {
            instance_id,
            item: entry.item,
            slot,
            ammo_item: weapon.ammo_item,
        },
        tuning: weapon.tuning,
        stored_state: state,
    })
}
