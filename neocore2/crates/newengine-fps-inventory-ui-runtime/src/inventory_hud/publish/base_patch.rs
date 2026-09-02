fn base_patch(
    frame_index: u64,
    state: &InventoryHudState,
    inventory: &PlayerInventory,
    mission: Option<&FpsObjectiveState>,
    weapon_state: Option<PlayerWeaponState>,
    vitals: Option<CharacterVitalsHudModel>,
    total_weight: f32,
    catalog: &ItemCatalog,
) -> UiStatePatch {
    let selected_entry = state
        .selected_instance
        .and_then(|instance| inventory.entry(instance));
    let selected_definition = selected_entry.and_then(|entry| catalog.get(entry.item));
    let capacity = inventory.capacity_state(catalog);
    let selected_equipped_slot = selected_entry.and_then(|entry| {
        inventory
            .equipped
            .iter()
            .find_map(|(slot, instance)| (*instance == entry.instance_id).then_some(*slot))
    });
    UiStatePatch::new(frame_index, INVENTORY_HUD_SURFACE_ID)
        .with_change("hud", "visible", serde_json::json!(state.visible))
        .with_change("inventory", "open", serde_json::json!(state.open))
        .with_change(
            "inventory",
            "weight_label",
            serde_json::json!(format!(
                "{total_weight:.1} / {:.1} kg",
                inventory.weight_capacity
            )),
        )
        .with_change(
            "inventory",
            "slots_label",
            serde_json::json!(format!(
                "{} / {} slots",
                capacity.used_slots, capacity.slot_capacity
            )),
        )
        .with_change(
            "inventory",
            "slots_normalized",
            serde_json::json!(capacity.slot_fill()),
        )
        .with_change(
            "inventory",
            "weight_normalized",
            serde_json::json!(capacity.weight_fill()),
        )
        .with_change(
            "inventory",
            "free_slots",
            serde_json::json!(capacity.free_slots()),
        )
        .with_change(
            "inventory",
            "free_weight",
            serde_json::json!(capacity.free_weight()),
        )
        .with_change(
            "inventory",
            "drag_active",
            serde_json::json!(state.drag.is_some()),
        )
        .with_change("mission", "visible", serde_json::json!(mission.is_some()))
        .with_change(
            "mission",
            "title",
            serde_json::json!(mission.map_or("", |mission| mission.title.as_str())),
        )
        .with_change(
            "mission",
            "objective",
            serde_json::json!(mission.map_or("", |mission| mission.objective.as_str())),
        )
        .with_change(
            "mission",
            "progress",
            serde_json::json!(mission
                .map(FpsObjectiveState::progress_label)
                .unwrap_or_default()),
        )
        .with_change(
            "mission",
            "status",
            serde_json::json!(mission.map_or("", |mission| mission.status.as_str())),
        )
        .with_change(
            "inventory",
            "selected_name",
            serde_json::json!(selected_definition
                .map(|definition| definition.display_name.as_str())
                .unwrap_or("")),
        )
        .with_change(
            "inventory",
            "selected_description",
            serde_json::json!(selected_definition
                .map(|definition| definition.description.as_str())
                .unwrap_or("")),
        )
        .with_change(
            "inventory",
            "selected_quantity",
            serde_json::json!(selected_entry.map_or(0, |entry| entry.quantity)),
        )
        .with_change(
            "inventory",
            "selected_condition",
            serde_json::json!(selected_entry.map_or(0.0, |entry| entry.condition)),
        )
        .with_change(
            "inventory",
            "selected_kind",
            serde_json::json!(selected_definition
                .map(|definition| item_kind_name(definition.kind))
                .unwrap_or("")),
        )
        .with_change(
            "inventory",
            "selected_unit_weight",
            serde_json::json!(selected_definition.map_or(0.0, |definition| definition.unit_weight)),
        )
        .with_change(
            "inventory",
            "selected_stack_weight",
            serde_json::json!(selected_entry
                .zip(selected_definition)
                .map_or(0.0, |(entry, definition)| definition.unit_weight
                    * entry.quantity as f32)),
        )
        .with_change(
            "inventory",
            "selected_equipped",
            serde_json::json!(selected_equipped_slot.is_some()),
        )
        .with_change(
            "inventory",
            "selected_equipment_slot",
            serde_json::json!(selected_equipped_slot
                .map(equipment_slot_name)
                .unwrap_or("")),
        )
        .with_change(
            "player",
            "ammo_label",
            serde_json::json!(weapon_state.map_or_else(
                || "-- / --".to_owned(),
                |weapon| format!("{} / {}", weapon.ammo_in_magazine, weapon.reserve_ammo)
            )),
        )
        .with_change(
            "player",
            "health_normalized",
            serde_json::json!(vitals.map_or(0.0, |vitals| vitals.health_normalized)),
        )
        .with_change(
            "player",
            "health_label",
            serde_json::json!(vitals.map_or_else(
                || "-- / --".to_owned(),
                |vitals| format!(
                    "{:.0} / {:.0}",
                    vitals.health_current, vitals.health_maximum
                )
            )),
        )
        .with_change(
            "player",
            "stamina_visible",
            serde_json::json!(vitals.is_some_and(|vitals| vitals.stamina_available)),
        )
        .with_change(
            "player",
            "stamina_normalized",
            serde_json::json!(vitals.map_or(0.0, |vitals| vitals.stamina_normalized)),
        )
        .with_change(
            "player",
            "stamina_label",
            serde_json::json!(vitals.map_or_else(
                || "-- / --".to_owned(),
                |vitals| format!(
                    "{:.0} / {:.0}",
                    vitals.stamina_current, vitals.stamina_maximum
                )
            )),
        )
        .with_change(
            "player",
            "stamina_exhausted",
            serde_json::json!(vitals.is_some_and(|vitals| vitals.stamina_exhausted)),
        )
        .with_change(
            "player",
            "injured",
            serde_json::json!(vitals.is_some_and(|vitals| vitals.injured)),
        )
        .with_change(
            "player",
            "damage_flash",
            serde_json::json!(vitals.is_some_and(|vitals| vitals.damage_flash)),
        )
        .with_change(
            "player",
            "dead",
            serde_json::json!(vitals.is_some_and(|vitals| !vitals.alive)),
        )
        .with_change(
            "player",
            "hit_reaction",
            serde_json::json!(vitals
                .map(|vitals| vitals.hit_reaction.as_str())
                .unwrap_or("none")),
        )
}
