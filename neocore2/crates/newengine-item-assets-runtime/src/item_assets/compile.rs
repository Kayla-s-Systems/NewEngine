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
            if let Some(firearm) = weapon.firearm {
                let ammo = catalog.get(firearm.ammo_item).ok_or_else(|| {
                    format!(
                        "firearm '{}' references missing ammo item {:016x}",
                        definition.name,
                        firearm.ammo_item.raw()
                    )
                })?;
                if ammo.kind != ItemKind::Ammo {
                    return Err(format!(
                        "firearm '{}' ammo reference '{}' is not kind=ammo",
                        definition.name, ammo.name
                    ));
                }
                if ammo.ammo_profile.is_none() {
                    return Err(format!(
                        "firearm '{}' ammo reference '{}' has no authored ammo_profile",
                        definition.name, ammo.name
                    ));
                }
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
            let authored_weapon = authored
                .weapon
                .as_ref()
                .ok_or_else(|| format!("weapon '{}' has no weapon definition", authored.id))?;
            let weapon_type = authored_weapon.weapon_type()?;
            let rank = authored_weapon.effective_rank(weapon_type);
            let weapon = match weapon_type {
                WeaponType::Unarmed => {
                    WeaponItemDefinition::unarmed(rank, authored_weapon.melee_tuning())
                }
                WeaponType::Melee => {
                    WeaponItemDefinition::melee(rank, authored_weapon.melee_tuning())
                }
                WeaponType::Firearm => {
                    let ammo_item = ItemId::from_name(&authored_weapon.ammo).ok_or_else(|| {
                        format!(
                            "firearm '{}' has invalid or missing ammo id '{}'",
                            authored.id, authored_weapon.ammo
                        )
                    })?;
                    let fire_mode = authored_weapon.fire_mode()?;
                    let pattern = authored_weapon.firing_pattern(fire_mode)?;
                    WeaponItemDefinition::firearm_with_pattern(
                        rank,
                        authored_weapon.tuning(),
                        ammo_item,
                        fire_mode,
                        pattern,
                    )
                }
            };
            let equipment_slot = if weapon_type == WeaponType::Unarmed
                && authored.equipment_slot.trim().is_empty()
            {
                None
            } else {
                Some(parse_equipment_slot(&authored.equipment_slot)?)
            };
            ItemDefinition::typed_weapon(
                &authored.id,
                display_name,
                equipment_slot,
                weapon,
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
    if kind == ItemKind::Ammo {
        let ammo = authored
            .ammo_profile
            .as_ref()
            .ok_or_else(|| format!("ammo '{}' has no ammo_profile", authored.id))?;
        definition = definition.with_ammo_profile(ammo.compile()?);
    } else if authored.ammo_profile.is_some() {
        return Err(format!(
            "item '{}' authors ammo_profile but kind is not ammo",
            authored.id
        ));
    }
    if kind != ItemKind::Weapon && !authored.equipment_slot.trim().is_empty() {
        definition.equipment_slot = Some(parse_equipment_slot(&authored.equipment_slot)?);
    }
    if let Some(components) = authored.weapon_components.as_ref() {
        definition = definition.with_weapon_components(components.compile()?)?;
    }
    if let Some(animation) = authored.weapon_animation.as_ref() {
        definition = definition.with_weapon_animation(animation.compile());
    }
    if let Some(audio) = authored.weapon_audio.as_ref() {
        definition = definition.with_weapon_audio(audio.compile());
    }
    if let Some(vfx) = authored.weapon_vfx.as_ref() {
        definition = definition.with_weapon_vfx(vfx.compile());
    }
    if let Some(presentation) = authored.weapon_presentation.as_ref() {
        definition = definition.with_weapon_presentation(presentation.compile());
    }
    if let Some(casing) = authored.weapon_casing.as_ref() {
        definition = definition.with_weapon_casing(casing.compile());
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
