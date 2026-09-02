use super::*;

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

pub fn active_equipped_weapon_component_stat_modifiers(
    world: &World,
    owner: EntityId,
) -> WeaponStatModifierStack {
    let Some(binding) = world.get::<EquippedWeaponBinding>(owner).copied() else {
        return WeaponStatModifierStack::default();
    };
    let Some(definition) = world
        .resource::<ItemCatalog>()
        .and_then(|catalog| catalog.get(binding.item))
    else {
        return WeaponStatModifierStack::default();
    };
    let Some(installed) = world
        .get::<PlayerInventory>(owner)
        .and_then(|inventory| inventory.weapon_components.get(&binding.instance_id))
    else {
        return WeaponStatModifierStack::default();
    };
    let mut modifiers = Vec::new();
    for instance in installed.values().filter(|instance| instance.active) {
        if let Some(component) = definition
            .weapon_components
            .components
            .get(&instance.component_id)
        {
            modifiers.extend(component.stat_modifiers.modifiers.iter().copied());
        }
    }
    WeaponStatModifierStack { modifiers }.sanitized()
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
