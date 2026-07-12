use super::*;

use super::*;
use crate::gameplay::{
    default_medkit_item_id, default_rifle_item_id, give_item, spawn_default_player,
};
use newengine_math::Vec3;

#[test]
fn f1_visibility_toggle_hides_entire_hud_and_closes_inventory() {
    let mut world = World::new();
    let player = spawn_default_player(&mut world, None, "hud-visibility-player", Vec3::ZERO);
    if let Some(state) = world.resource_mut::<InventoryHudState>() {
        state.open = true;
    }
    if let Some(commands) = world.get_mut::<PlayerCommandFrame>(player) {
        commands.actions.hud_visibility_toggle_pressed = true;
    }

    step_inventory_commands(&mut world, 1);

    let hidden = world
        .resource::<InventoryHudState>()
        .expect("inventory HUD");
    assert!(!hidden.visible);
    assert!(!hidden.open);

    if let Some(commands) = world.get_mut::<PlayerCommandFrame>(player) {
        commands.actions.clear_pulses();
        commands.actions.hud_visibility_toggle_pressed = true;
    }
    step_inventory_commands(&mut world, 2);
    assert!(inventory_hud_is_visible(&world));
}

#[test]
fn command_toggle_and_hotbar_selection_update_inventory_hud() {
    let mut world = World::new();
    let player = spawn_default_player(&mut world, None, "hud-player", Vec3::ZERO);
    if let Some(commands) = world.get_mut::<PlayerCommandFrame>(player) {
        commands.actions.inventory_toggle_pressed = true;
        commands.actions.equipment_slot_pressed = Some(3);
    }
    step_inventory_commands(&mut world, 1);
    assert!(inventory_hud_is_open(&world));
    assert_eq!(
        world
            .get::<PlayerInventory>(player)
            .expect("inventory")
            .active_slot,
        Some(EquipmentSlot::Sidearm)
    );
}

#[test]
fn drag_reorders_stable_instances_and_equips_matching_target() {
    let mut world = World::new();
    let player = spawn_default_player(&mut world, None, "drag-player", Vec3::ZERO);
    let rifle = world
        .get::<PlayerInventory>(player)
        .expect("inventory")
        .entries
        .iter()
        .find(|entry| entry.item == default_rifle_item_id())
        .expect("rifle")
        .instance_id;
    let initial = world
        .get::<PlayerInventory>(player)
        .expect("inventory")
        .entries
        .iter()
        .position(|entry| entry.instance_id == rifle)
        .expect("position");
    reorder_inventory(&mut world, player, rifle, 5);
    let moved = world
        .get::<PlayerInventory>(player)
        .expect("inventory")
        .entries
        .iter()
        .position(|entry| entry.instance_id == rifle)
        .expect("position");
    assert_ne!(initial, moved);
    equip_dragged_instance(&mut world, player, rifle, EquipmentSlot::Primary);
    assert_eq!(
        world
            .get::<PlayerInventory>(player)
            .expect("inventory")
            .equipped_instance(EquipmentSlot::Primary),
        Some(rifle)
    );
}

#[test]
fn double_click_consumable_uses_item_and_drop_creates_world_pickup() {
    let mut world = World::new();
    let player = spawn_default_player(&mut world, None, "use-player", Vec3::ZERO);
    let medkit = default_medkit_item_id();
    let _ = give_item(&mut world, player, medkit, 1).expect("give medkit");
    if let Some(health) = world.get_mut::<crate::gameplay::Health>(player) {
        health.current = 10.0;
    }
    let instance = world
        .get::<PlayerInventory>(player)
        .expect("inventory")
        .entries
        .iter()
        .find(|entry| entry.item == medkit)
        .expect("medkit")
        .instance_id;
    activate_inventory_instance(&mut world, player, instance);
    assert!(
        world
            .get::<crate::gameplay::Health>(player)
            .unwrap()
            .current
            > 10.0
    );
    let rifle_instance = world
        .get::<PlayerInventory>(player)
        .unwrap()
        .entries
        .iter()
        .find(|entry| entry.item == default_rifle_item_id())
        .unwrap()
        .instance_id;
    drop_instance_quantity(&mut world, player, rifle_instance, 1);
    assert!(world
        .query::<crate::gameplay::ItemPickup>()
        .next()
        .is_some());
}
