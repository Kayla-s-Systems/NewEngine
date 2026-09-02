fn patch_inventory_slot(
    patch: UiStatePatch,
    index: usize,
    state: &InventoryHudState,
    inventory: &PlayerInventory,
    catalog: &ItemCatalog,
) -> UiStatePatch {
    let source = format!("inv_slot_{index:02}");
    let Some(entry) = inventory.entries.get(index) else {
        return patch_slot_fields(
            patch,
            &source,
            state.open,
            "".to_owned(),
            0,
            "",
            "empty",
            String::new(),
            false,
            false,
            false,
        );
    };

    let definition = catalog.get(entry.item);
    let equipped_slot = inventory
        .equipped
        .iter()
        .find_map(|(slot, instance)| (*instance == entry.instance_id).then_some(*slot));
    let name = definition
        .map(|definition| definition.display_name.as_str())
        .unwrap_or("Unknown Item");
    let label = if entry.quantity > 1 {
        format!("{name}  x{}", entry.quantity)
    } else {
        name.to_owned()
    };
    patch_slot_fields(
        patch,
        &source,
        state.open,
        label,
        entry.quantity,
        definition
            .and_then(|definition| definition.icon_ref.as_deref())
            .unwrap_or(""),
        definition.map_or("generic", |definition| item_kind_name(definition.kind)),
        entry.instance_id.0.to_string(),
        equipped_slot.is_some(),
        equipped_slot.is_some() && equipped_slot == inventory.active_slot,
        state.selected_instance == Some(entry.instance_id),
    )
}

#[allow(clippy::too_many_arguments)]
fn patch_slot_fields(
    patch: UiStatePatch,
    source: &str,
    visible: bool,
    label: String,
    quantity: u32,
    icon: &str,
    kind: &str,
    instance_id: String,
    equipped: bool,
    active: bool,
    selected: bool,
) -> UiStatePatch {
    patch
        .with_change(source, "visible", serde_json::json!(visible))
        .with_change(source, "label", serde_json::json!(label))
        .with_change(source, "quantity", serde_json::json!(quantity))
        .with_change(source, "icon", serde_json::json!(icon))
        .with_change(source, "kind", serde_json::json!(kind))
        .with_change(source, "instance_id", serde_json::json!(instance_id))
        .with_change(source, "equipped", serde_json::json!(equipped))
        .with_change(source, "active", serde_json::json!(active))
        .with_change(source, "selected", serde_json::json!(selected))
}

fn patch_hotbar_slot(
    patch: UiStatePatch,
    index: u8,
    inventory: &PlayerInventory,
    catalog: &ItemCatalog,
) -> UiStatePatch {
    let slot = hotbar_slot(index).expect("bounded hotbar slot");
    let source = format!("hotbar_{index}");
    let instance = inventory.equipped_instance(slot);
    let definition = instance
        .and_then(|instance| inventory.entry(instance))
        .and_then(|entry| catalog.get(entry.item));
    patch
        .with_change(
            &source,
            "label",
            serde_json::json!(definition
                .map(|definition| format!("{index}  {}", definition.display_name))
                .unwrap_or_else(|| format!("{index}  —"))),
        )
        .with_change(
            &source,
            "active",
            serde_json::json!(inventory.active_slot == Some(slot)),
        )
        .with_change(&source, "visible", serde_json::json!(instance.is_some()))
        .with_change(
            &source,
            "icon",
            serde_json::json!(definition
                .and_then(|definition| definition.icon_ref.as_deref())
                .unwrap_or("")),
        )
}

fn patch_equipment_slot(
    patch: UiStatePatch,
    slot: EquipmentSlot,
    visible: bool,
    inventory: &PlayerInventory,
    catalog: &ItemCatalog,
) -> UiStatePatch {
    let source = format!("equip_{}", equipment_slot_name(slot));
    let definition = inventory
        .equipped_instance(slot)
        .and_then(|instance| inventory.entry(instance))
        .and_then(|entry| catalog.get(entry.item));
    patch
        .with_change(&source, "visible", serde_json::json!(visible))
        .with_change(
            &source,
            "label",
            serde_json::json!(
                definition.map_or("Empty", |definition| definition.display_name.as_str())
            ),
        )
        .with_change(
            &source,
            "active",
            serde_json::json!(inventory.active_slot == Some(slot)),
        )
}

#[inline]
fn item_kind_name(kind: ItemKind) -> &'static str {
    match kind {
        ItemKind::Generic => "generic",
        ItemKind::Weapon => "weapon",
        ItemKind::Ammo => "ammo",
        ItemKind::Consumable => "consumable",
        ItemKind::Component => "component",
        ItemKind::Quest => "quest",
        ItemKind::Key => "key",
    }
}
