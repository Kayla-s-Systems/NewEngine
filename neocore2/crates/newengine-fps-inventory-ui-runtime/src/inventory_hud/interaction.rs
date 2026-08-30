use super::*;

pub fn apply_inventory_ui_actions(world: &mut World, frame: &UiEventDispatchFrame) -> bool {
    ensure_inventory_hud_state(world);
    let Some(player) = first_player(world) else {
        return false;
    };
    sync_character_nav_from_retained_focus(world, frame);
    let mut consumed = false;
    for action in frame
        .actions
        .iter()
        .filter(|action| action.surface_id == INVENTORY_HUD_SURFACE_ID)
    {
        if action.trigger == UiNodeEventTrigger::Click {
            if let Some(item) = parse_registered_weapon_action(&action.action_id) {
                consumed |= select_registered_weapon(world, player, item);
                continue;
            }
        }
        match action.action_id.as_str() {
            INVENTORY_UI_ACTION_TOGGLE => {
                if action.trigger == UiNodeEventTrigger::Click {
                    toggle_inventory(world);
                    consumed = true;
                }
            }
            INVENTORY_UI_ACTION_HOTBAR => {
                if action.trigger == UiNodeEventTrigger::Click {
                    if let Some(index) = parse_hotbar_index(&action.node_id) {
                        if let Some(slot) = hotbar_slot(index) {
                            let _ = select_equipment_slot(world, player, slot);
                            touch_hud_state(world);
                            consumed = true;
                        }
                    }
                }
            }
            INVENTORY_UI_ACTION_EQUIPMENT => {
                if action.trigger == UiNodeEventTrigger::Click {
                    if let Some(slot) = parse_equipment_node(&action.node_id) {
                        let _ = select_equipment_slot(world, player, slot);
                        touch_hud_state(world);
                        consumed = true;
                    }
                }
            }
            INVENTORY_UI_ACTION_SLOT => {
                consumed |= handle_slot_action(world, player, frame, action);
            }
            INVENTORY_UI_ACTION_DROP => {
                if matches!(
                    action.trigger,
                    UiNodeEventTrigger::DragEnd | UiNodeEventTrigger::Click
                ) {
                    consumed |= drop_selected_or_dragged(world, player);
                }
            }
            CHARACTER_UI_ACTION_TOGGLE => {
                if action.trigger == UiNodeEventTrigger::Click {
                    if let Some(state) = world.resource_mut::<InventoryHudState>() {
                        state.toggle_character_select();
                    }
                    consumed = true;
                }
            }
            CHARACTER_UI_ACTION_CATEGORY_CHARACTERS => {
                if action.trigger == UiNodeEventTrigger::Click {
                    if let Some(state) = world.resource_mut::<InventoryHudState>() {
                        state.set_character_category(CharacterMenuCategory::Characters);
                    }
                    consumed = true;
                }
            }
            CHARACTER_UI_ACTION_CATEGORY_WEAPONS => {
                if action.trigger == UiNodeEventTrigger::Click {
                    if let Some(state) = world.resource_mut::<InventoryHudState>() {
                        state.set_character_category(CharacterMenuCategory::Weapons);
                    }
                    consumed = true;
                }
            }
            CHARACTER_UI_ACTION_NOCLIP_TOGGLE => {
                if action.trigger == UiNodeEventTrigger::Click {
                    if toggle_fps_noclip(world, player) {
                        touch_hud_state(world);
                    }
                    consumed = true;
                }
            }
            _ => {
                if action.trigger == UiNodeEventTrigger::Click {
                    if let Some(variant) = variant_from_action(world, &action.action_id).cloned() {
                        suppress_character_selector_pointer_leak(world, player);
                        consumed |= select_playable_character(world, player, &variant);
                    }
                }
            }
        }
    }
    consumed
}

const REGISTERED_WEAPON_ACTION_PREFIX: &str = "game.weapon.select.";

fn parse_registered_weapon_action(action_id: &str) -> Option<ItemId> {
    let raw = action_id.strip_prefix(REGISTERED_WEAPON_ACTION_PREFIX)?;
    raw.parse::<u64>().ok().map(ItemId)
}

fn select_registered_weapon(world: &mut World, player: EntityId, item: ItemId) -> bool {
    let Some(definition) = world
        .resource::<ItemCatalog>()
        .and_then(|catalog| catalog.get(item))
        .cloned()
    else {
        return false;
    };
    if definition.kind != ItemKind::Weapon {
        return false;
    }

    if definition.name == SHARED_UNARMED_WEAPON_ITEM_NAME || definition.equipment_slot.is_none() {
        if let Some(inventory) = world.get_mut::<PlayerInventory>(player) {
            inventory.active_slot = None;
        }
        sync_equipped_weapon_runtime(world, player);
        touch_hud_state(world);
        newengine_ulog_api::ulog::info!(
            "player options weapon selected registration_name='{}' item={:016x} mode='unarmed'",
            definition.name,
            item.0,
        );
        return true;
    }

    let owned = world
        .get::<PlayerInventory>(player)
        .is_some_and(|inventory| inventory.quantity(item) > 0);
    if !owned && give_item(world, player, item, 1).is_err() {
        return false;
    }
    if equip_first_item(world, player, item).is_err() {
        return false;
    }
    touch_hud_state(world);
    newengine_ulog_api::ulog::info!(
        "player options weapon selected registration_name='{}' item={:016x} source='registered-item-catalog'",
        definition.name,
        item.0,
    );
    true
}

fn sync_character_nav_from_retained_focus(world: &mut World, frame: &UiEventDispatchFrame) {
    if !character_select_is_open(world) {
        return;
    }
    let Some(focused) = frame
        .focused_node
        .as_ref()
        .filter(|focused| focused.surface_id == INVENTORY_HUD_SURFACE_ID)
    else {
        return;
    };
    let variants = playable_character_variants(world);
    let noclip_index = variants.len();
    let target_index = if focused.action_id.as_deref() == Some(CHARACTER_UI_ACTION_NOCLIP_TOGGLE) {
        Some(noclip_index)
    } else {
        focused.action_id.as_deref().and_then(|action_id| {
            let variant_id = action_id.strip_prefix("game.character.select.")?;
            variants.iter().position(|variant| variant.id == variant_id)
        })
    };
    if let Some(index) = target_index {
        if let Some(state) = world.resource_mut::<InventoryHudState>() {
            state.set_character_nav_index(index, noclip_index + 1);
        }
    }
}

pub(super) fn handle_slot_action(
    world: &mut World,
    player: EntityId,
    frame: &UiEventDispatchFrame,
    action: &newengine_ui_api::UiActionDispatch,
) -> bool {
    let Some(source_index) = parse_inventory_slot_index(world, &action.node_id) else {
        return false;
    };
    let source_instance = inventory_instance_at(world, player, source_index);
    match action.trigger {
        UiNodeEventTrigger::Click => {
            if let Some(state) = world.resource_mut::<InventoryHudState>() {
                state.selected_instance = source_instance;
                state.touch();
            }
            true
        }
        UiNodeEventTrigger::DoubleClick => {
            let Some(instance) = source_instance else {
                return true;
            };
            activate_inventory_instance(world, player, instance);
            true
        }
        UiNodeEventTrigger::ContextMenu => {
            let Some(instance) = source_instance else {
                return true;
            };
            drop_instance_quantity(world, player, instance, 1);
            true
        }
        UiNodeEventTrigger::DragStart => {
            if let Some(instance_id) = source_instance {
                if let Some(state) = world.resource_mut::<InventoryHudState>() {
                    state.drag = Some(InventoryDragState {
                        instance_id,
                        source_index,
                    });
                    state.selected_instance = Some(instance_id);
                    state.touch();
                }
            }
            true
        }
        UiNodeEventTrigger::DragMove => true,
        UiNodeEventTrigger::DragEnd => {
            let target_node = frame
                .hovered_node
                .as_ref()
                .map(|hit| hit.node_id.as_str())
                .unwrap_or_default();
            finish_drag(world, player, target_node);
            true
        }
        _ => false,
    }
}

pub(super) fn finish_drag(world: &mut World, player: EntityId, target_node: &str) {
    let drag = world
        .resource::<InventoryHudState>()
        .and_then(|state| state.drag);
    let Some(drag) = drag else {
        return;
    };
    if let Some(target_index) = parse_inventory_slot_index(world, target_node) {
        reorder_inventory(world, player, drag.instance_id, target_index);
    } else if let Some(target_slot) = parse_equipment_node(target_node) {
        equip_dragged_instance(world, player, drag.instance_id, target_slot);
    } else if target_node == "inventory.drop.zone" {
        drop_instance_quantity(world, player, drag.instance_id, 1);
    }
    if let Some(state) = world.resource_mut::<InventoryHudState>() {
        state.drag = None;
        state.touch();
    }
}

pub(super) fn reorder_inventory(
    world: &mut World,
    player: EntityId,
    instance: ItemInstanceId,
    target_index: usize,
) {
    let Some(inventory) = world.get_mut::<PlayerInventory>(player) else {
        return;
    };
    let Some(source_index) = inventory
        .entries
        .iter()
        .position(|entry| entry.instance_id == instance)
    else {
        return;
    };
    if source_index == target_index || source_index >= inventory.entries.len() {
        return;
    }
    let entry = inventory.entries.remove(source_index);
    let adjusted = if source_index < target_index {
        target_index.saturating_sub(1)
    } else {
        target_index
    };
    let insertion = adjusted.min(inventory.entries.len());
    inventory.entries.insert(insertion, entry);
    touch_hud_state(world);
}

pub(super) fn equip_dragged_instance(
    world: &mut World,
    player: EntityId,
    instance: ItemInstanceId,
    target_slot: EquipmentSlot,
) {
    let allowed_slot = world
        .get::<PlayerInventory>(player)
        .and_then(|inventory| inventory.entry(instance))
        .and_then(|entry| world.resource::<ItemCatalog>()?.get(entry.item))
        .and_then(|definition| definition.equipment_slot);
    if allowed_slot == Some(target_slot) {
        let _ = equip_item_instance(world, player, instance);
        touch_hud_state(world);
    }
}

pub(super) fn activate_inventory_instance(
    world: &mut World,
    player: EntityId,
    instance: ItemInstanceId,
) {
    let item = world
        .get::<PlayerInventory>(player)
        .and_then(|inventory| inventory.entry(instance))
        .map(|entry| entry.item);
    let Some(item) = item else {
        return;
    };
    let definition = world
        .resource::<ItemCatalog>()
        .and_then(|catalog| catalog.get(item))
        .cloned();
    let Some(definition) = definition else {
        return;
    };
    match definition.kind {
        ItemKind::Consumable => {
            let _ = use_item(world, player, item);
        }
        _ if definition.equipment_slot.is_some() => {
            let _ = equip_item_instance(world, player, instance);
        }
        _ => {}
    }
    touch_hud_state(world);
}

pub(super) fn drop_selected_or_dragged(world: &mut World, player: EntityId) -> bool {
    let instance = world.resource::<InventoryHudState>().and_then(|state| {
        state
            .drag
            .map(|drag| drag.instance_id)
            .or(state.selected_instance)
    });
    let Some(instance) = instance else {
        return false;
    };
    drop_instance_quantity(world, player, instance, 1);
    true
}

pub(super) fn drop_instance_quantity(
    world: &mut World,
    player: EntityId,
    instance: ItemInstanceId,
    quantity: u32,
) {
    let item = world
        .get::<PlayerInventory>(player)
        .and_then(|inventory| inventory.entry(instance))
        .map(|entry| entry.item);
    if let Some(item) = item {
        let _ = drop_item(world, player, item, quantity);
        if world
            .get::<PlayerInventory>(player)
            .and_then(|inventory| inventory.entry(instance))
            .is_none()
        {
            if let Some(state) = world.resource_mut::<InventoryHudState>() {
                if state.selected_instance == Some(instance) {
                    state.selected_instance = None;
                }
            }
        }
        touch_hud_state(world);
    }
}

fn suppress_character_selector_pointer_leak(world: &mut World, player: EntityId) {
    let Some(commands) = world.get_mut::<PlayerCommandFrame>(player) else {
        return;
    };
    // The same physical left click that selects a retained-mode UI button can already
    // be present in the semantic gameplay command frame. Because selection closes the
    // selector immediately, the subsequent FPS phase must not reinterpret that click
    // as fire/projectile/aim input.
    commands.actions.held.retain(|action| {
        action != newengine_gameplay_fps_api::action::PLAYER_FIRE_PRIMARY
            && action != newengine_gameplay_fps_api::action::PLAYER_AIM
    });
    commands.actions.pressed.retain(|action| {
        action != newengine_gameplay_fps_api::action::PLAYER_FIRE_PRIMARY
            && action != newengine_gameplay_fps_api::action::PLAYER_LAUNCH_PROJECTILE
    });
}

pub(super) fn select_playable_character(
    world: &mut World,
    player: EntityId,
    variant: &FpsPlayableCharacterPolicy,
) -> bool {
    // Selection is terminal for this modal: close first so changing avatar never
    // leaves the character picker covering the newly selected character.
    if let Some(state) = world.resource_mut::<InventoryHudState>() {
        state.close_character_select();
        newengine_ulog_api::ulog::info!(
            "character selector closing for selection variant={}",
            variant.id
        );
    }
    let Some(assignment) = character_variants::assignment(variant) else {
        newengine_ulog_api::ulog::warn!(
            "playable character variant is not runtime-ready id={} family={} availability={} source={}",
            variant.id,
            variant.family,
            availability_label(variant),
            variant.source_provenance,
        );
        return false;
    };
    match newengine_engine_runtime::gameplay::set_player_model_assignment(world, player, assignment)
    {
        Ok(revision) => {
            let _ = world.insert(
                player,
                PlayableCharacterSelection {
                    variant_id: variant.id.to_owned(),
                },
            );
            newengine_ulog_api::ulog::info!(
                "playable character selected variant={} family={} rig={} player={} revision={}",
                variant.id,
                variant.family,
                variant.rig_label,
                player.stable_u64(),
                revision
            );
            true
        }
        Err(error) => {
            newengine_ulog_api::ulog::warn!(
                "playable character selection rejected variant={} player={}: {}",
                variant.id,
                player.stable_u64(),
                error
            );
            false
        }
    }
}
