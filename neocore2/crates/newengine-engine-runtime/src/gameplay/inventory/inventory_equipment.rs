use super::operations::emit_inventory_event;
use super::*;

pub fn preload_weapon_audio_definition(audio: &WeaponAudioDefinition) {
    for action in [
        WeaponAudioAction::Fire,
        WeaponAudioAction::ReloadStart,
        WeaponAudioAction::ReloadComplete,
        WeaponAudioAction::Equip,
        WeaponAudioAction::Unequip,
        WeaponAudioAction::Empty,
        WeaponAudioAction::ShellEject,
    ] {
        let Some(reference) = audio.clip(action) else {
            continue;
        };
        let result = if is_yscd_cue_reference(reference) {
            newengine_audio_client::preload_audio_cue(
                &newengine_audio_api::AudioCuePreloadRequest {
                    cue: newengine_audio_api::SoundCueRef::new(reference.to_owned()),
                },
            )
        } else {
            newengine_audio_client::preload_audio_clip(&newengine_audio_api::AudioPreloadRequest {
                clip: newengine_audio_api::AudioClipRef::new(reference.to_owned()),
            })
        };
        match result {
            Ok(Some(ack)) if ack.accepted => {
                newengine_ulog_api::ulog::info!(
                    "weapon audio preload: action={:?} ref='{}' kind='{}' provider='{}' bytes={} cached={} status='ready'",
                    action,
                    reference,
                    if is_yscd_cue_reference(reference) { "yscd-cue" } else { "clip" },
                    ack.provider,
                    ack.bytes,
                    ack.cached,
                );
                for diagnostic in &ack.diagnostics {
                    newengine_ulog_api::ulog::info!("{}", diagnostic);
                }
            }
            Ok(Some(ack)) => newengine_ulog_api::ulog::warn!(
                "weapon audio preload rejected: action={:?} ref='{}' provider='{}'",
                action,
                reference,
                ack.provider,
            ),
            Ok(None) => newengine_ulog_api::ulog::warn!(
                "weapon audio preload unavailable: action={:?} ref='{}' reason='engine.audio returned no provider response'",
                action,
                reference,
            ),
            Err(error) => newengine_ulog_api::ulog::warn!(
                "weapon audio preload failed: action={:?} ref='{}' err='{}'",
                action,
                reference,
                error,
            ),
        }
    }
}

#[inline]
fn is_yscd_cue_reference(reference: &str) -> bool {
    newengine_assets_api::parse_asset_reference(reference)
        .map(|reference| {
            reference.has_extension("yscd")
                && reference
                    .entry
                    .as_deref()
                    .is_some_and(|entry| !entry.trim().is_empty())
        })
        .unwrap_or(false)
}

pub fn play_weapon_item_audio(
    world: &World,
    owner: EntityId,
    item: ItemId,
    action: WeaponAudioAction,
) {
    let Some(reference) = world
        .resource::<ItemCatalog>()
        .and_then(|catalog| catalog.get(item))
        .and_then(|definition| definition.weapon_audio.clip(action))
        .map(ToOwned::to_owned)
    else {
        return;
    };
    let spatial_position = match action {
        WeaponAudioAction::Fire | WeaponAudioAction::ShellEject => world
            .get::<EquippedWeaponMuzzle>(owner)
            .map(|muzzle| muzzle.position)
            .or_else(|| {
                world
                    .get::<Transform>(owner)
                    .map(|transform| transform.position)
            }),
        _ => world
            .get::<Transform>(owner)
            .map(|transform| transform.position),
    };

    let is_cue = is_yscd_cue_reference(&reference);
    let result = if is_cue {
        let mut request = newengine_audio_api::AudioCuePlayRequest::new(reference.clone());
        request.position = spatial_position.map(|position| [position.x, position.y, position.z]);
        newengine_audio_client::play_audio_cue(&request)
    } else {
        let mut request = newengine_audio_api::AudioPlayRequest::new(reference.clone());
        request.spatial =
            spatial_position.map(|position| newengine_audio_api::AudioSpatialParams {
                position: [position.x, position.y, position.z],
            });
        newengine_audio_client::play_audio_clip(&request)
    };

    match result {
        Ok(Some(ack)) if ack.accepted => {
            if matches!(action, WeaponAudioAction::Fire | WeaponAudioAction::Empty) {
                newengine_ulog_api::ulog::info!(
                    "weapon audio play: action={:?} ref='{}' kind='{}' provider='{}' voice_id={:?} virtualized={} status='accepted'",
                    action,
                    reference,
                    if is_cue { "yscd-cue" } else { "clip" },
                    ack.provider,
                    ack.voice_id,
                    ack.virtualized,
                );
            }
            for diagnostic in &ack.diagnostics {
                newengine_ulog_api::ulog::info!("{}", diagnostic);
            }
        }
        Ok(Some(ack)) => newengine_ulog_api::ulog::warn!(
            "weapon audio play rejected: action={:?} ref='{}' provider='{}' message='{}'",
            action,
            reference,
            ack.provider,
            ack.message,
        ),
        Ok(None) => newengine_ulog_api::ulog::warn!(
            "weapon audio play unavailable: action={:?} ref='{}' reason='engine.audio returned no provider response'",
            action,
            reference,
        ),
        Err(error) => newengine_ulog_api::ulog::warn!(
            "weapon audio play failed: action={:?} ref='{}' err='{}'",
            action,
            reference,
            error,
        ),
    }
}

pub fn play_equipped_weapon_audio(world: &World, owner: EntityId, action: WeaponAudioAction) {
    let Some(binding) = world.get::<EquippedWeaponBinding>(owner) else {
        return;
    };
    play_weapon_item_audio(world, owner, binding.item, action);
}

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
        play_weapon_item_audio(world, owner, previous_item, WeaponAudioAction::Unequip);
    }
    if definition.kind == ItemKind::Weapon {
        play_weapon_item_audio(world, owner, item, WeaponAudioAction::Equip);
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
            play_weapon_item_audio(world, owner, previous_item, WeaponAudioAction::Unequip);
        }
        play_weapon_item_audio(world, owner, item, WeaponAudioAction::Equip);
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
    play_weapon_item_audio(world, owner, item, WeaponAudioAction::Unequip);
    Ok(())
}

pub fn sync_equipped_weapon_runtime(world: &mut World, owner: EntityId) {
    let selected = selected_weapon(world, owner);
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
