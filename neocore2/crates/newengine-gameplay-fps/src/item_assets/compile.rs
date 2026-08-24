use super::*;

pub fn parse_authored_item_package_json(bytes: &[u8]) -> Result<AuthoredItemPackage, String> {
    let package: AuthoredItemPackage = serde_json::from_slice(bytes)
        .map_err(|error| format!("authored item package JSON parse failed: {error}"))?;
    validate_package_header(&package)?;
    Ok(package)
}

pub fn compile_authored_item_package(
    package: &AuthoredItemPackage,
) -> Result<CompiledItemPackage, String> {
    validate_package_header(package)?;
    let mut catalog = ItemCatalog::default();

    for authored in &package.items {
        let definition = compile_item_definition(authored)?;
        catalog.register(definition)?;
    }

    for definition in catalog.definitions() {
        if let Some(weapon) = definition.weapon {
            let ammo = catalog.get(weapon.ammo_item).ok_or_else(|| {
                format!(
                    "weapon '{}' references missing ammo item {:016x}",
                    definition.name,
                    weapon.ammo_item.raw()
                )
            })?;
            if ammo.kind != ItemKind::Ammo {
                return Err(format!(
                    "weapon '{}' ammo reference '{}' is not kind=ammo",
                    definition.name, ammo.name
                ));
            }
        }
    }

    let mut loadouts = InventoryLoadoutCatalog::default();
    for authored in &package.loadouts {
        let mut loadout = InventoryLoadout::new(&authored.id)?;
        loadout.name = if authored.display_name.trim().is_empty() {
            authored.id.trim().to_owned()
        } else {
            authored.display_name.trim().to_owned()
        };
        loadout.clear_existing = authored.clear_existing;
        for entry in &authored.entries {
            let definition = catalog.find(&entry.item).ok_or_else(|| {
                format!(
                    "loadout '{}' references missing item '{}'",
                    authored.id, entry.item
                )
            })?;
            loadout.entries.push(InventoryLoadoutEntry {
                item: definition.id,
                quantity: entry.quantity,
                equip: entry.equip,
            });
        }
        loadouts.register(loadout)?;
    }

    Ok(CompiledItemPackage { catalog, loadouts })
}

pub fn install_compiled_item_package(world: &mut World, package: CompiledItemPackage) {
    for definition in package.catalog.definitions() {
        if definition.kind == ItemKind::Weapon {
            preload_weapon_audio_definition(&definition.weapon_audio);
        }
    }
    world.insert_resource(package.catalog);
    world.insert_resource(package.loadouts);
    if world.resource::<InventoryEventBus>().is_none() {
        world.insert_resource(InventoryEventBus::default());
    }
}

fn compile_item_definition(authored: &AuthoredItemDefinition) -> Result<ItemDefinition, String> {
    let kind = parse_item_kind(&authored.kind)?;
    let display_name = if authored.display_name.trim().is_empty() {
        authored.id.trim()
    } else {
        authored.display_name.trim()
    };
    let mut definition = match kind {
        ItemKind::Weapon => {
            let weapon = authored
                .weapon
                .as_ref()
                .ok_or_else(|| format!("weapon '{}' has no weapon definition", authored.id))?;
            let ammo_item = ItemId::from_name(&weapon.ammo).ok_or_else(|| {
                format!(
                    "weapon '{}' has invalid ammo id '{}'",
                    authored.id, weapon.ammo
                )
            })?;
            ItemDefinition::weapon(
                &authored.id,
                display_name,
                parse_equipment_slot(&authored.equipment_slot)?,
                weapon.tuning(),
                ammo_item,
                weapon.fire_mode()?,
                authored.unit_weight,
            )?
        }
        ItemKind::Consumable => ItemDefinition::consumable(
            &authored.id,
            display_name,
            authored.max_stack,
            authored.unit_weight,
            parse_use_effect(authored.use_effect.as_ref())?,
        )?,
        other => ItemDefinition::stackable(
            &authored.id,
            display_name,
            other,
            authored.max_stack,
            authored.unit_weight,
        )?,
    };
    definition = definition
        .with_description(authored.description.trim())
        .with_tags(authored.tags.clone());
    if !authored.definition_ref.trim().is_empty() {
        definition = definition.with_definition_ref(authored.definition_ref.trim());
    }
    if !authored.icon.trim().is_empty() {
        definition = definition.with_icon(authored.icon.trim());
    }
    if kind != ItemKind::Weapon && !authored.equipment_slot.trim().is_empty() {
        definition.equipment_slot = Some(parse_equipment_slot(&authored.equipment_slot)?);
    }
    if let Some(audio) = authored.weapon_audio.as_ref() {
        definition = definition.with_weapon_audio(audio.compile());
    }
    if let Some(world) = authored.world.as_ref() {
        definition = definition.with_world_definition(world.compile(kind)?);
    }
    Ok(definition)
}

pub(super) fn validate_package_header(package: &AuthoredItemPackage) -> Result<(), String> {
    if package.schema != AUTHORED_ITEM_PACKAGE_SCHEMA {
        return Err(format!(
            "item package schema mismatch: got='{}' expected='{}'",
            package.schema, AUTHORED_ITEM_PACKAGE_SCHEMA
        ));
    }
    if package.version != AUTHORED_ITEM_PACKAGE_VERSION {
        return Err(format!(
            "item package version mismatch: got={} expected={}",
            package.version, AUTHORED_ITEM_PACKAGE_VERSION
        ));
    }
    if package.items.is_empty() {
        return Err("item package must contain at least one item".to_owned());
    }
    Ok(())
}

fn parse_item_kind(value: &str) -> Result<ItemKind, String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "generic" => Ok(ItemKind::Generic),
        "weapon" => Ok(ItemKind::Weapon),
        "ammo" | "ammunition" => Ok(ItemKind::Ammo),
        "consumable" => Ok(ItemKind::Consumable),
        "component" => Ok(ItemKind::Component),
        "quest" => Ok(ItemKind::Quest),
        "key" => Ok(ItemKind::Key),
        other => Err(format!("unsupported item kind '{other}'")),
    }
}

fn parse_equipment_slot(value: &str) -> Result<EquipmentSlot, String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "primary" => Ok(EquipmentSlot::Primary),
        "secondary" => Ok(EquipmentSlot::Secondary),
        "sidearm" => Ok(EquipmentSlot::Sidearm),
        "melee" => Ok(EquipmentSlot::Melee),
        "throwable" => Ok(EquipmentSlot::Throwable),
        "gadget" => Ok(EquipmentSlot::Gadget),
        "utility1" | "utility_1" => Ok(EquipmentSlot::Utility1),
        "utility2" | "utility_2" => Ok(EquipmentSlot::Utility2),
        other => Err(format!("unsupported equipment slot '{other}'")),
    }
}

fn parse_use_effect(effect: Option<&AuthoredUseEffect>) -> Result<ItemUseEffect, String> {
    let Some(effect) = effect else {
        return Ok(ItemUseEffect::None);
    };
    match effect.kind.trim().to_ascii_lowercase().as_str() {
        "" | "none" => Ok(ItemUseEffect::None),
        "heal" => Ok(ItemUseEffect::Heal {
            amount: effect.amount.max(0.0),
        }),
        other => Err(format!("unsupported item use effect '{other}'")),
    }
}
