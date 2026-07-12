use super::*;

#[inline]
pub fn default_rifle_item_id() -> ItemId {
    ItemId::from_name(DEFAULT_RIFLE_ITEM_NAME).expect("valid built-in item name")
}

#[inline]
pub fn default_rifle_ammo_id() -> ItemId {
    ItemId::from_name(DEFAULT_RIFLE_AMMO_NAME).expect("valid built-in ammo name")
}

#[inline]
pub fn default_medkit_item_id() -> ItemId {
    ItemId::from_name(DEFAULT_MEDKIT_ITEM_NAME).expect("valid built-in item name")
}

#[inline]
pub fn default_fps_loadout_id() -> ItemId {
    ItemId::from_name(DEFAULT_FPS_LOADOUT_NAME).expect("valid built-in loadout name")
}

pub fn ensure_default_item_catalog(world: &mut World) {
    if world.resource::<ItemCatalog>().is_none()
        || world.resource::<InventoryLoadoutCatalog>().is_none()
    {
        match crate::gameplay::item_assets::compiled_embedded_fps_item_package() {
            Ok(package) => {
                crate::gameplay::item_assets::install_compiled_item_package(world, package)
            }
            Err(error) => {
                newengine_ulog_api::ulog::warn!(
                    "inventory: authored embedded NEITEMS package failed; using built-in fallback err='{}'",
                    error
                );
                if world.resource::<ItemCatalog>().is_none() {
                    world.insert_resource(ItemCatalog::fps_defaults());
                }
                if world.resource::<InventoryLoadoutCatalog>().is_none() {
                    world.insert_resource(InventoryLoadoutCatalog::fps_defaults());
                }
            }
        }
    }
    if world.resource::<InventoryEventBus>().is_none() {
        world.insert_resource(InventoryEventBus::default());
    }
}

pub fn ensure_player_inventory(world: &mut World, owner: EntityId) {
    ensure_default_item_catalog(world);
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
    let catalog = world
        .resource::<ItemCatalog>()
        .cloned()
        .ok_or_else(|| "item catalog is unavailable".to_owned())?;
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
        .ok_or_else(|| "loadout definition is unavailable".to_owned())?;
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

pub fn give_default_fps_loadout(world: &mut World, owner: EntityId) -> Result<(), String> {
    apply_loadout(world, owner, default_fps_loadout_id())
}

pub fn drain_inventory_events(world: &mut World) -> Vec<InventoryEvent> {
    world
        .resource_mut::<InventoryEventBus>()
        .map(InventoryEventBus::drain)
        .unwrap_or_default()
}

pub(super) fn emit_inventory_event(world: &mut World, event: InventoryEvent) {
    if world.resource::<InventoryEventBus>().is_none() {
        world.insert_resource(InventoryEventBus::default());
    }
    if let Some(bus) = world.resource_mut::<InventoryEventBus>() {
        bus.emit(event);
    }
}
