use super::operations::emit_inventory_event;
use super::*;
use crate::gameplay::{
    emit_gameplay_event, GAMEPLAY_EVENT_WEAPON_EQUIPPED, GAMEPLAY_EVENT_WEAPON_UNEQUIPPED,
};

/// Resolves the active weapon's physical muzzle pose.
///
/// `EquippedWeaponEntity -> WeaponEntitySockets` is authoritative. The owner-side
/// `EquippedWeaponMuzzle` is only a compatibility projection from the same weapon presentation;
/// camera/body synthesis is deliberately forbidden here.
pub fn active_equipped_weapon_muzzle(
    world: &World,
    owner: EntityId,
) -> Option<EquippedWeaponMuzzle> {
    if let Some(binding) = world.get::<EquippedWeaponBinding>(owner).copied() {
        if let Some(link) = world
            .get::<EquippedWeaponEntity>(owner)
            .copied()
            .filter(|link| link.instance_id == binding.instance_id && link.item == binding.item)
        {
            if let Some(socket) = world
                .get::<WeaponEntitySockets>(link.entity)
                .and_then(|sockets| sockets.muzzle)
            {
                let forward = (socket.rotation * Vec3::Z).normalize_or_zero();
                if let Some(muzzle) = EquippedWeaponMuzzle::new(socket.position, forward) {
                    return Some(muzzle);
                }
            }
        }
    }

    world.get::<EquippedWeaponMuzzle>(owner).copied()
}

pub fn active_equipped_weapon_component_modifiers(
    world: &World,
    owner: EntityId,
) -> WeaponComponentModifiers {
    let Some(binding) = world.get::<EquippedWeaponBinding>(owner).copied() else {
        return WeaponComponentModifiers::default();
    };
    let Some(definition) = world
        .resource::<ItemCatalog>()
        .and_then(|catalog| catalog.get(binding.item))
    else {
        return WeaponComponentModifiers::default();
    };
    let Some(installed) = world
        .get::<PlayerInventory>(owner)
        .and_then(|inventory| inventory.weapon_components.get(&binding.instance_id))
    else {
        return WeaponComponentModifiers::default();
    };
    installed.values().filter(|instance| instance.active).fold(
        WeaponComponentModifiers::default(),
        |combined, instance| {
            definition
                .weapon_components
                .components
                .get(&instance.component_id)
                .map(|component| combined.combine(component.modifiers))
                .unwrap_or(combined)
        },
    )
}

pub fn active_equipped_weapon_component_overrides(
    world: &World,
    owner: EntityId,
) -> (Option<String>, Option<String>, Option<String>) {
    let Some(binding) = world.get::<EquippedWeaponBinding>(owner).copied() else {
        return (None, None, None);
    };
    let Some(definition) = world
        .resource::<ItemCatalog>()
        .and_then(|catalog| catalog.get(binding.item))
    else {
        return (None, None, None);
    };
    let Some(installed) = world
        .get::<PlayerInventory>(owner)
        .and_then(|inventory| inventory.weapon_components.get(&binding.instance_id))
    else {
        return (None, None, None);
    };
    let mut audio = None;
    let mut muzzle = None;
    let mut tracer = None;
    for instance in installed.values().filter(|instance| instance.active) {
        let Some(component) = definition
            .weapon_components
            .components
            .get(&instance.component_id)
        else {
            continue;
        };
        if component.audio_override.is_some() {
            audio = component.audio_override.clone();
        }
        if component.muzzle_vfx_override.is_some() {
            muzzle = component.muzzle_vfx_override.clone();
        }
        if component.tracer_vfx_override.is_some() {
            tracer = component.tracer_vfx_override.clone();
        }
    }
    (audio, muzzle, tracer)
}

pub fn install_weapon_component(
    world: &mut World,
    owner: EntityId,
    weapon_instance: ItemInstanceId,
    slot: &str,
    component_id: &str,
) -> Result<(), String> {
    let slot = slot.trim().to_ascii_lowercase();
    let component_id = component_id.trim().to_ascii_lowercase();
    let item = world
        .get::<PlayerInventory>(owner)
        .and_then(|inventory| inventory.entry(weapon_instance))
        .map(|entry| entry.item)
        .ok_or_else(|| "weapon instance is not present in inventory".to_owned())?;
    let graph = world
        .resource::<ItemCatalog>()
        .and_then(|catalog| catalog.get(item))
        .map(|definition| definition.weapon_components.clone().sanitized())
        .ok_or_else(|| "weapon definition is unavailable".to_owned())?;
    let point = graph
        .points
        .iter()
        .find(|point| point.id == slot)
        .ok_or_else(|| format!("unknown weapon component slot '{slot}'"))?;
    let component = graph
        .components
        .get(&component_id)
        .ok_or_else(|| format!("unknown weapon component '{component_id}'"))?;
    if component.slot != slot
        || (!point.allowed_components.is_empty()
            && !point.allowed_components.contains(&component_id))
    {
        return Err(format!(
            "component '{component_id}' is not allowed in slot '{slot}'"
        ));
    }
    let inventory = world
        .get_mut::<PlayerInventory>(owner)
        .ok_or_else(|| "owner has no inventory".to_owned())?;
    inventory
        .weapon_components
        .entry(weapon_instance)
        .or_default()
        .insert(
            slot,
            WeaponComponentInstance {
                component_id,
                active: true,
            },
        );
    Ok(())
}

pub fn remove_weapon_component(
    world: &mut World,
    owner: EntityId,
    weapon_instance: ItemInstanceId,
    slot: &str,
) -> Option<WeaponComponentInstance> {
    world
        .get_mut::<PlayerInventory>(owner)?
        .weapon_components
        .get_mut(&weapon_instance)?
        .remove(&slot.trim().to_ascii_lowercase())
}

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

/// Legacy audio helpers are kept as a compatibility API for external clients. The engine
/// equipment/combat path no longer calls them directly; projects subscribe to semantic
/// gameplay events and choose their own cues/actions.
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
    let component_audio_override = world
        .get::<EquippedWeaponBinding>(owner)
        .copied()
        .filter(|binding| binding.item == item)
        .and_then(|_| active_equipped_weapon_component_overrides(world, owner).0);
    let Some(reference) = component_audio_override.or_else(|| {
        world
            .resource::<ItemCatalog>()
            .and_then(|catalog| catalog.get(item))
            .and_then(|definition| definition.weapon_audio.clip(action))
            .map(ToOwned::to_owned)
    }) else {
        return;
    };
    let component_gain = world
        .get::<EquippedWeaponBinding>(owner)
        .copied()
        .filter(|binding| binding.item == item)
        .map(|_| active_equipped_weapon_component_modifiers(world, owner).audio_gain_multiplier)
        .unwrap_or(1.0);
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
        request.gain = component_gain;
        request.position = spatial_position.map(|position| [position.x, position.y, position.z]);
        request.scope_id = Some(owner.stable_u64());
        newengine_audio_client::play_audio_cue(&request)
    } else {
        let mut request = newengine_audio_api::AudioPlayRequest::new(reference.clone());
        request.gain = component_gain;
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

#[cfg(test)]
mod component_graph_tests {
    use super::*;

    fn component(
        id: &str,
        slot: &str,
        accuracy: f32,
        recoil: f32,
        damage: f32,
    ) -> WeaponComponentDefinition {
        WeaponComponentDefinition {
            id: id.to_owned(),
            slot: slot.to_owned(),
            model_ref: None,
            audio_override: None,
            muzzle_vfx_override: None,
            tracer_vfx_override: None,
            modifiers: WeaponComponentModifiers {
                accuracy_multiplier: accuracy,
                recoil_multiplier: recoil,
                damage_multiplier: damage,
                ..WeaponComponentModifiers::default()
            },
        }
    }

    #[test]
    fn component_install_validates_slot_and_aggregates_active_instance_modifiers() {
        let mut world = World::new();
        let owner = world.spawn();
        let ammo = ItemId::from_name("ammo.component.test").expect("ammo id");
        let weapon_id = ItemId::from_name("weapon.component.test").expect("weapon id");

        let graph = WeaponComponentGraphDefinition {
            points: vec![
                WeaponComponentPointDefinition {
                    id: "muzzle".to_owned(),
                    attach_joint: "muzzle".to_owned(),
                    allowed_components: vec![
                        "muzzle.standard".to_owned(),
                        "muzzle.brake".to_owned(),
                    ],
                },
                WeaponComponentPointDefinition {
                    id: "optic".to_owned(),
                    attach_joint: "optic".to_owned(),
                    allowed_components: vec!["optic.basic".to_owned()],
                },
            ],
            components: [
                (
                    "muzzle.standard".to_owned(),
                    component("muzzle.standard", "muzzle", 1.0, 1.0, 1.0),
                ),
                (
                    "muzzle.brake".to_owned(),
                    component("muzzle.brake", "muzzle", 0.9, 0.7, 1.05),
                ),
                (
                    "optic.basic".to_owned(),
                    component("optic.basic", "optic", 0.8, 1.0, 1.0),
                ),
            ]
            .into_iter()
            .collect(),
            default_installed: [("muzzle".to_owned(), "muzzle.standard".to_owned())]
                .into_iter()
                .collect(),
        };

        let weapon = ItemDefinition::weapon(
            "weapon.component.test",
            "Component Test Weapon",
            EquipmentSlot::Primary,
            HitscanWeaponTuning::default(),
            ammo,
            WeaponFireMode::SemiAuto,
            2.5,
        )
        .expect("weapon")
        .with_weapon_components(graph)
        .expect("component graph");
        let mut catalog = ItemCatalog::default();
        catalog.register(weapon).expect("register weapon");
        world.insert_resource(catalog);

        let mutation = give_item(&mut world, owner, weapon_id, 1).expect("give weapon");
        let instance = *mutation.touched_instances.first().expect("weapon instance");
        equip_item_instance(&mut world, owner, instance).expect("equip weapon");

        let defaults = active_equipped_weapon_component_modifiers(&world, owner);
        assert!((defaults.recoil_multiplier - 1.0).abs() < 1.0e-6);

        assert!(
            install_weapon_component(&mut world, owner, instance, "muzzle", "optic.basic").is_err(),
            "component from another slot must be rejected"
        );
        install_weapon_component(&mut world, owner, instance, "muzzle", "muzzle.brake")
            .expect("install muzzle brake");

        let modified = active_equipped_weapon_component_modifiers(&world, owner);
        assert!((modified.accuracy_multiplier - 0.9).abs() < 1.0e-6);
        assert!((modified.recoil_multiplier - 0.7).abs() < 1.0e-6);
        assert!((modified.damage_multiplier - 1.05).abs() < 1.0e-6);

        let removed = remove_weapon_component(&mut world, owner, instance, "muzzle")
            .expect("remove component");
        assert_eq!(removed.component_id, "muzzle.brake");
        assert_eq!(
            active_equipped_weapon_component_modifiers(&world, owner),
            WeaponComponentModifiers::default()
        );
    }
}
