use super::*;

pub(super) fn publish_inventory_hud_state(
    world: &mut World,
    frame_index: u64,
) -> GameplayUiFrameOutput {
    ensure_inventory_hud_state(world);
    let Some(player) = first_player(world) else {
        return GameplayUiFrameOutput::default();
    };
    let fingerprint = inventory_hud_fingerprint(world, player);
    if world
        .resource::<InventoryHudState>()
        .is_some_and(|state| state.last_published_hash == fingerprint)
    {
        return GameplayUiFrameOutput::default();
    }

    let (patch, visible) = {
        let state = world
            .resource::<InventoryHudState>()
            .expect("inventory HUD state initialized");
        let inventory = world.get::<PlayerInventory>(player);
        let catalog = world.resource::<ItemCatalog>();
        let mission = world.resource::<FpsDemoState>();
        let weapon_state = world.get::<PlayerWeaponState>(player).copied();
        let binding = world.get::<EquippedWeaponBinding>(player).copied();

        let empty_inventory = PlayerInventory::default();
        let empty_catalog = ItemCatalog::default();
        let inventory = inventory.unwrap_or(&empty_inventory);
        let catalog = catalog.unwrap_or(&empty_catalog);
        let total_weight = inventory.total_weight(catalog);

        let mut patch = base_patch(
            frame_index,
            state,
            inventory,
            mission,
            weapon_state,
            total_weight,
            catalog,
        );
        for index in 0..inventory_slot_count(world) {
            patch = patch_inventory_slot(patch, index, state, inventory, catalog);
        }
        for index in 1..=5u8 {
            patch = patch_hotbar_slot(patch, index, inventory, catalog);
        }
        for slot in EQUIPMENT_SLOTS {
            patch = patch_equipment_slot(patch, slot, state.open, inventory, catalog);
        }
        if let Some(binding) = binding {
            patch = patch.with_change(
                "inventory",
                "active_weapon_instance",
                serde_json::json!(binding.instance_id.0.to_string()),
            );
        }
        (patch, state.visible)
    };

    if let Some(state) = world.resource_mut::<InventoryHudState>() {
        state.last_published_hash = fingerprint;
        state.last_published_frame = frame_index;
    }

    GameplayUiFrameOutput::default()
        .with_patch(patch, "gameplay.fps.inventory", INVENTORY_HUD_CONTRACT)
        .with_surface_visibility(INVENTORY_HUD_SURFACE_ID, visible)
}

fn base_patch(
    frame_index: u64,
    state: &InventoryHudState,
    inventory: &PlayerInventory,
    mission: Option<&FpsDemoState>,
    weapon_state: Option<PlayerWeaponState>,
    total_weight: f32,
    catalog: &ItemCatalog,
) -> UiStatePatch {
    let selected_definition = state.selected_instance.and_then(|instance| {
        let entry = inventory.entry(instance)?;
        catalog.get(entry.item)
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
                inventory.used_slots(),
                inventory.slot_capacity
            )),
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
                .map(FpsDemoState::progress_label)
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
            "player",
            "ammo_label",
            serde_json::json!(weapon_state.map_or_else(
                || "-- / --".to_owned(),
                |weapon| format!("{} / {}", weapon.ammo_in_magazine, weapon.reserve_ammo)
            )),
        )
}

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

pub(super) fn inventory_hud_fingerprint(world: &World, player: EntityId) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    let mut push = |value: u64| {
        hash ^= value;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    };
    if let Some(state) = world.resource::<InventoryHudState>() {
        push(state.visible as u64);
        push(state.open as u64);
        push(state.revision);
        push(state.selected_instance.map_or(0, |instance| instance.0));
        push(state.drag.map_or(0, |drag| drag.instance_id.0));
    }
    if let Some(inventory) = world.get::<PlayerInventory>(player) {
        push(inventory.entries.len() as u64);
        push(inventory.active_slot.map_or(0, equipment_slot_code));
        for entry in &inventory.entries {
            push(entry.instance_id.0);
            push(entry.item.0);
            push(u64::from(entry.quantity));
            push(u64::from(entry.condition.to_bits()));
        }
        for (slot, instance) in &inventory.equipped {
            push(equipment_slot_code(*slot));
            push(instance.0);
        }
    }
    if let Some(weapon) = world.get::<PlayerWeaponState>(player) {
        push(u64::from(weapon.ammo_in_magazine));
        push(u64::from(weapon.reserve_ammo));
        push(weapon.shot_sequence);
    }
    if let Some(mission) = world.resource::<FpsDemoState>() {
        push(u64::from(mission.pickups_collected));
        push(u64::from(mission.pickups_total));
        push(u64::from(mission.targets_destroyed));
        push(u64::from(mission.targets_total));
        push(mission.completed as u64);
        push(mission.failed as u64);
        for byte in mission.status.as_bytes() {
            push(u64::from(*byte));
        }
    }
    hash
}
