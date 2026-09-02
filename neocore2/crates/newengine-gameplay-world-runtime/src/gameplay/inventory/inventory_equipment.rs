use super::operations::emit_inventory_event;
use super::*;
use crate::gameplay::{
    emit_gameplay_event, reconcile_character_injury_state, CharacterLifeState,
    GAMEPLAY_EVENT_CHARACTER_HEALED, GAMEPLAY_EVENT_WEAPON_EQUIPPED,
    GAMEPLAY_EVENT_WEAPON_UNEQUIPPED,
};

#[path = "inventory_equipment/components.rs"]
mod components;
pub use components::{
    active_equipped_weapon_component_modifiers, active_equipped_weapon_component_overrides,
    active_equipped_weapon_component_stat_modifiers, active_equipped_weapon_muzzle,
    install_weapon_component, remove_weapon_component,
};

fn publish_weapon_equipment_event(
    world: &mut World,
    id: &str,
    owner: EntityId,
    item: ItemId,
    instance_id: Option<ItemInstanceId>,
) {
    let Some(definition) = world
        .resource::<ItemCatalog>()
        .and_then(|catalog| catalog.get(item))
    else {
        return;
    };
    if definition.kind != ItemKind::Weapon {
        return;
    }
    let weapon_name = definition.name.clone();
    let position = world.get::<Transform>(owner).map(|transform| {
        [
            transform.position.x,
            transform.position.y,
            transform.position.z,
        ]
    });
    let payload = serde_json::json!({
        "schema": "newengine.gameplay.weapon_equipment_event.v1",
        "version": 1,
        "weapon_item_id": item.raw(),
        "weapon": weapon_name,
        "weapon_instance_id": instance_id.map(|instance| instance.0),
        "position": position,
    });
    if let Err(error) = emit_gameplay_event(world, id, Some(owner), payload) {
        newengine_ulog_api::ulog::warn!(
            "weapon equipment semantic event rejected: event='{}' owner={} err='{}'",
            id,
            owner.stable_u64(),
            error,
        );
    }
}

#[path = "inventory_equipment/audio_compat.rs"]
mod audio_compat;
pub use audio_compat::{
    play_equipped_weapon_audio, play_weapon_item_audio, preload_weapon_audio_definition,
};

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
    if let Some(previous_item) = previous_item.filter(|previous_item| *previous_item != item) {
        publish_weapon_equipment_event(
            world,
            GAMEPLAY_EVENT_WEAPON_UNEQUIPPED,
            owner,
            previous_item,
            previous,
        );
    }
    if definition.kind == ItemKind::Weapon {
        publish_weapon_equipment_event(
            world,
            GAMEPLAY_EVENT_WEAPON_EQUIPPED,
            owner,
            item,
            Some(instance),
        );
    }
    Ok(())
}

pub fn select_equipment_slot(
    world: &mut World,
    owner: EntityId,
    slot: EquipmentSlot,
) -> Result<(), String> {
    persist_equipped_weapon_state(world, owner);
    let previous_item = world
        .get::<EquippedWeaponBinding>(owner)
        .map(|binding| binding.item);
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
    if previous_item != Some(item) {
        if let Some(previous_item) = previous_item {
            publish_weapon_equipment_event(
                world,
                GAMEPLAY_EVENT_WEAPON_UNEQUIPPED,
                owner,
                previous_item,
                None,
            );
        }
        publish_weapon_equipment_event(
            world,
            GAMEPLAY_EVENT_WEAPON_EQUIPPED,
            owner,
            item,
            Some(instance),
        );
    }
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
    let needs_weapon_selection = world
        .get::<PlayerInventory>(owner)
        .is_some_and(|inventory| inventory.active_slot.is_none());
    if needs_weapon_selection {
        select_highest_ranked_equipped_weapon(world, owner);
    }
    sync_equipped_weapon_runtime(world, owner);
    publish_weapon_equipment_event(
        world,
        GAMEPLAY_EVENT_WEAPON_UNEQUIPPED,
        owner,
        item,
        Some(removed),
    );
    Ok(())
}

pub fn sync_equipped_weapon_runtime(world: &mut World, owner: EntityId) {
    let selected = selected_weapon(world, owner);
    let (weapon_mode_event, weapon_type) = selected
        .as_ref()
        .map(|selected| match selected.binding.weapon.weapon_type {
            WeaponType::Unarmed => ("character.weapon.mode.unarmed", "unarmed"),
            WeaponType::Melee => ("character.weapon.mode.melee", "melee"),
            WeaponType::Firearm => ("character.weapon.mode.firearm", "firearm"),
        })
        .unwrap_or(("character.weapon.mode.none", "none"));
    if let Err(error) = super::super::emit_animation_state(
        world,
        owner,
        "character.weapon.mode",
        weapon_mode_event,
        serde_json::json!({"weapon_type": weapon_type}),
    ) {
        newengine_ulog_api::ulog::warn!(
            "weapon animation semantic mode publish failed owner={} err='{}'",
            owner.stable_u64(),
            error
        );
    }
    let current_binding = world.get::<EquippedWeaponBinding>(owner).copied();
    let current_state = world.get::<PlayerWeaponState>(owner).copied();

    let selected_instance = selected
        .as_ref()
        .map(|selected| selected.binding.instance_id);
    if current_binding.map(|binding| binding.instance_id) != selected_instance {
        if let (Some(binding), Some(mut state)) = (current_binding, current_state) {
            state.aiming = false;
            if !binding.is_unarmed() {
                if let Some(inventory) = world.get_mut::<PlayerInventory>(owner) {
                    if inventory.entry(binding.instance_id).is_some() {
                        inventory.weapon_states.insert(binding.instance_id, state);
                    }
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

    let reserve = selected
        .binding
        .weapon
        .firearm
        .map(|firearm| inventory_quantity(world, owner, firearm.ammo_item))
        .unwrap_or(0);

    if current_binding == Some(selected.binding) {
        if let Some(firearm) = selected.binding.weapon.firearm {
            if world.get::<HitscanWeaponTuning>(owner).is_none() {
                let _ = world.insert(owner, firearm.tuning);
            }
        } else {
            let _ = world.remove::<HitscanWeaponTuning>(owner);
        }
        if let Some(state) = world.get_mut::<PlayerWeaponState>(owner) {
            state.reserve_ammo = reserve;
            state.aiming &= selected.binding.capabilities().aim;
        } else {
            let mut state = selected.stored_state;
            state.reserve_ammo = reserve;
            let _ = world.insert(owner, state);
        }
        return;
    }

    let mut state = selected.stored_state;
    state.reserve_ammo = reserve;
    state.aiming = false;
    let _ = world.insert(owner, selected.binding);
    if let Some(firearm) = selected.binding.weapon.firearm {
        let _ = world.insert(owner, firearm.tuning);
    } else {
        let _ = world.remove::<HitscanWeaponTuning>(owner);
    }
    let _ = world.insert(owner, state);
}

pub fn persist_equipped_weapon_state(world: &mut World, owner: EntityId) {
    let Some(binding) = world.get::<EquippedWeaponBinding>(owner).copied() else {
        return;
    };
    if binding.is_unarmed() {
        return;
    }
    let Some(mut state) = world.get::<PlayerWeaponState>(owner).copied() else {
        return;
    };
    state.aiming = false;
    if let Some(inventory) = world.get_mut::<PlayerInventory>(owner) {
        if inventory.entry(binding.instance_id).is_some() {
            inventory.weapon_states.insert(binding.instance_id, state);
        }
    }
}

/// Returns the authoritative weapon context. When no inventory weapon is selected, this is the
/// virtual Unarmed weapon rather than absence of a weapon.
pub fn active_equipped_weapon_binding(
    world: &World,
    owner: EntityId,
) -> Option<EquippedWeaponBinding> {
    let selected = selected_weapon(world, owner)?;
    match world.get::<EquippedWeaponBinding>(owner).copied() {
        Some(binding) if binding == selected.binding => Some(binding),
        None if selected.binding.is_unarmed() => Some(selected.binding),
        _ => None,
    }
}

#[inline]
pub fn active_equipped_weapon_aiming(world: &World, owner: EntityId) -> bool {
    active_equipped_weapon_binding(world, owner).is_some_and(|binding| {
        binding.capabilities().aim
            && world
                .get::<PlayerWeaponState>(owner)
                .is_some_and(|state| state.aiming)
    })
}

#[inline]
pub fn active_equipped_weapon_can_aim(world: &World, owner: EntityId) -> bool {
    active_equipped_weapon_binding(world, owner).is_some_and(|binding| binding.capabilities().aim)
}

#[inline]
pub fn active_equipped_weapon_can_fire(world: &World, owner: EntityId) -> bool {
    active_equipped_weapon_binding(world, owner).is_some_and(|binding| binding.capabilities().fire)
}

#[inline]
pub fn active_equipped_weapon_can_melee(world: &World, owner: EntityId) -> bool {
    active_equipped_weapon_binding(world, owner).is_some_and(|binding| binding.capabilities().melee)
}

pub fn equipped_reserve_ammo(world: &World, owner: EntityId) -> Option<u32> {
    let binding = active_equipped_weapon_binding(world, owner)?;
    let firearm = binding.weapon.firearm?;
    Some(inventory_quantity(world, owner, firearm.ammo_item))
}

pub fn consume_equipped_ammo(world: &mut World, owner: EntityId, requested: u32) -> u32 {
    let Some(binding) = active_equipped_weapon_binding(world, owner) else {
        return 0;
    };
    let Some(firearm) = binding.weapon.firearm else {
        return 0;
    };
    let mutation = world
        .get_mut::<PlayerInventory>(owner)
        .map(|inventory| inventory.remove_quantity(firearm.ammo_item, requested))
        .unwrap_or_default();
    if mutation.accepted > 0 {
        emit_inventory_event(
            world,
            InventoryEvent {
                kind: InventoryEventKind::AmmoConsumed,
                owner,
                item: firearm.ammo_item,
                instance_id: mutation.touched_instances.last().copied(),
                quantity: mutation.accepted,
                slot: binding.slot,
                world_entity: None,
                message: "ammunition transferred into magazine".to_owned(),
            },
        );
    }
    mutation.accepted
}

pub fn use_item(world: &mut World, owner: EntityId, item: ItemId) -> Result<(), String> {
    use_item_internal(world, owner, item, None)
}

pub fn use_item_instance(
    world: &mut World,
    owner: EntityId,
    instance: ItemInstanceId,
) -> Result<(), String> {
    let item = world
        .get::<PlayerInventory>(owner)
        .and_then(|inventory| inventory.entry(instance))
        .map(|entry| entry.item)
        .ok_or_else(|| "inventory instance is not present".to_owned())?;
    use_item_internal(world, owner, item, Some(instance))
}

fn use_item_internal(
    world: &mut World,
    owner: EntityId,
    item: ItemId,
    exact_instance: Option<ItemInstanceId>,
) -> Result<(), String> {
    let definition = world
        .resource::<ItemCatalog>()
        .and_then(|catalog| catalog.get(item))
        .cloned()
        .ok_or_else(|| "item definition is unavailable".to_owned())?;
    let present = match exact_instance {
        Some(instance) => world
            .get::<PlayerInventory>(owner)
            .and_then(|inventory| inventory.entry(instance))
            .is_some_and(|entry| entry.item == item && entry.quantity > 0),
        None => inventory_quantity(world, owner, item) > 0,
    };
    if !present {
        return Err("item is not present in inventory".to_owned());
    }

    match definition.use_effect {
        ItemUseEffect::None => return Err("item has no usable effect".to_owned()),
        ItemUseEffect::Heal { amount } => {
            if world
                .get::<CharacterLifeState>(owner)
                .is_some_and(|state| !state.alive())
            {
                return Err("dead character requires an explicit revive mechanic".to_owned());
            }
            let applied = {
                let health = world
                    .get_mut::<Health>(owner)
                    .ok_or_else(|| "owner has no health component".to_owned())?;
                if health.current >= health.maximum || amount <= 0.0 {
                    return Err("healing item would have no effect".to_owned());
                }
                health.heal(amount)
            };
            if applied > 0.0 {
                let health = world.get::<Health>(owner).copied().unwrap_or_default();
                let _ = emit_gameplay_event(
                    world,
                    GAMEPLAY_EVENT_CHARACTER_HEALED,
                    Some(owner),
                    serde_json::json!({
                        "item": item.0,
                        "applied_healing": applied,
                        "health_current": health.current,
                        "health_maximum": health.maximum,
                        "health_normalized": health.normalized(),
                    }),
                );
                let _ = reconcile_character_injury_state(world, owner);
            }
        }
    }

    let removed = world
        .get_mut::<PlayerInventory>(owner)
        .map(|inventory| match exact_instance {
            Some(instance) => inventory.remove_instance_quantity(instance, 1),
            None => inventory.remove_quantity(item, 1),
        })
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
            instance_id: exact_instance.or_else(|| removed.touched_instances.last().copied()),
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
    stored_state: PlayerWeaponState,
}

impl SelectedWeapon {
    fn shared_unarmed(world: &World) -> Option<Self> {
        let definition = world
            .resource::<ItemCatalog>()?
            .find(SHARED_UNARMED_WEAPON_ITEM_NAME)?;
        let weapon = definition.weapon?;
        if weapon.weapon_type != WeaponType::Unarmed || weapon.melee.is_none() {
            return None;
        }
        Some(Self {
            binding: EquippedWeaponBinding {
                instance_id: ItemInstanceId::UNARMED,
                item: definition.id,
                slot: None,
                weapon,
            },
            stored_state: PlayerWeaponState::melee(),
        })
    }
}

fn selected_weapon(world: &World, owner: EntityId) -> Option<SelectedWeapon> {
    let selected_inventory_weapon = world.get::<PlayerInventory>(owner).and_then(|inventory| {
        inventory.active_slot.and_then(|slot| {
            let instance_id = inventory.equipped_instance(slot)?;
            let entry = inventory.entry(instance_id)?;
            let definition = world.resource::<ItemCatalog>()?.get(entry.item)?;
            let weapon = definition.weapon?;
            let mut state = inventory
                .weapon_states
                .get(&instance_id)
                .copied()
                .unwrap_or_else(|| {
                    weapon
                        .firearm
                        .map(|firearm| PlayerWeaponState::loaded(firearm.tuning))
                        .unwrap_or_else(PlayerWeaponState::melee)
                });
            state.reserve_ammo = weapon
                .firearm
                .map(|firearm| inventory.quantity(firearm.ammo_item))
                .unwrap_or(0);
            state.aiming = false;
            Some(SelectedWeapon {
                binding: EquippedWeaponBinding {
                    instance_id,
                    item: entry.item,
                    slot: Some(slot),
                    weapon,
                },
                stored_state: state,
            })
        })
    });

    selected_inventory_weapon.or_else(|| SelectedWeapon::shared_unarmed(world))
}

fn highest_ranked_equipped_weapon_slot(world: &World, owner: EntityId) -> Option<EquipmentSlot> {
    let inventory = world.get::<PlayerInventory>(owner)?;
    let catalog = world.resource::<ItemCatalog>()?;
    inventory
        .equipped
        .iter()
        .filter_map(|(slot, instance_id)| {
            let entry = inventory.entry(*instance_id)?;
            let weapon = catalog.get(entry.item)?.weapon?;
            Some((*slot, weapon.rank))
        })
        .max_by(|(slot_a, rank_a), (slot_b, rank_b)| {
            rank_a.cmp(rank_b).then_with(|| slot_b.cmp(slot_a))
        })
        .map(|(slot, _)| slot)
}

pub fn select_highest_ranked_equipped_weapon(
    world: &mut World,
    owner: EntityId,
) -> Option<EquipmentSlot> {
    let selected = highest_ranked_equipped_weapon_slot(world, owner);
    if let Some(inventory) = world.get_mut::<PlayerInventory>(owner) {
        inventory.active_slot = selected;
    }
    selected
}
#[cfg(test)]
#[path = "inventory_equipment/tests.rs"]
mod component_graph_tests;
