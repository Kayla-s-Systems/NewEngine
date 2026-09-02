fn registered_character_items(world: &World, player: EntityId) -> Vec<serde_json::Value> {
    let selected_id = selected_variant(world, player).map(|variant| variant.id.as_str());
    playable_character_variants(world)
        .iter()
        .map(|variant| {
            serde_json::json!({
                "entity_key": variant.id,
                "entity": variant.id,
                "display_name": variant.display_name,
                "action_id": format!("game.character.select.{}", variant.id),
                "subtitle": variant.subtitle,
                "family": variant.family,
                "rig_label": variant.rig_label,
                "status": availability_label(variant),
                "detail": format!("{} | {} | {}", variant.subtitle, variant.rig_label, availability_label(variant)),
                "reasons": [variant.subtitle, variant.rig_label],
                "runtime_ready": variant.runtime_ready,
                "disabled": !variant.runtime_ready,
                "selected": selected_id.is_some_and(|selected| selected == variant.id),
                "registration_name": variant.id,
            })
        })
        .collect()
}

fn registered_weapon_items(
    catalog: &ItemCatalog,
    active: Option<EquippedWeaponBinding>,
) -> Vec<serde_json::Value> {
    let mut weapons = catalog
        .definitions()
        .filter(|definition| definition.kind == ItemKind::Weapon)
        .collect::<Vec<_>>();
    weapons.sort_by(|left, right| left.name.cmp(&right.name));
    weapons
        .into_iter()
        .map(|definition| {
            let slot = definition
                .equipment_slot
                .map(equipment_slot_name)
                .unwrap_or("unassigned");
            let detail = if definition.description.trim().is_empty() {
                format!("slot: {slot}")
            } else {
                format!("{} · slot: {slot}", definition.description.trim())
            };
            let selected = active.is_some_and(|binding| binding.item == definition.id)
                || (active.is_none()
                    && (definition.name == SHARED_UNARMED_WEAPON_ITEM_NAME
                        || definition.equipment_slot.is_none()));
            serde_json::json!({
                "entity_key": definition.id.0,
                "entity": definition.name,
                "display_name": definition.display_name,
                "action_id": format!("game.weapon.select.{}", definition.id.0),
                "detail": detail,
                "reasons": [detail],
                "selected": selected,
                "disabled": false,
                "registration_name": definition.name,
            })
        })
        .collect()
}
