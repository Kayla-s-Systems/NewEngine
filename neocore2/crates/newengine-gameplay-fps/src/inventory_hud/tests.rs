use super::*;
use newengine_engine_runtime::gameplay::{
    apply_loadout, give_item, spawn_default_player, EquipmentSlot, HitscanWeaponTuning,
    InventoryLoadout, InventoryLoadoutCatalog, InventoryLoadoutEntry, ItemCatalog, ItemDefinition,
    ItemId, ItemKind, ItemUseEffect,
};
use newengine_math::Vec3;

const TEST_AMMO_NAME: &str = "test.ammo.primary";
const TEST_PRIMARY_NAME: &str = "test.weapon.primary";
const TEST_SIDEARM_NAME: &str = "test.weapon.sidearm";
const TEST_MEDKIT_NAME: &str = "test.consumable.medkit";
const TEST_LOADOUT_NAME: &str = "test.loadout.hud";

fn item_id(name: &str) -> ItemId {
    ItemId::from_name(name).expect("valid test item id")
}

fn install_test_content(world: &mut World) {
    let ammo = ItemDefinition::stackable(TEST_AMMO_NAME, "Test Ammo", ItemKind::Ammo, 240, 0.01)
        .expect("test ammo");
    let ammo_id = ammo.id;
    let primary = ItemDefinition::weapon(
        TEST_PRIMARY_NAME,
        "Test Primary",
        EquipmentSlot::Primary,
        HitscanWeaponTuning::default(),
        ammo_id,
        1.0,
    )
    .expect("test primary");
    let sidearm = ItemDefinition::weapon(
        TEST_SIDEARM_NAME,
        "Test Sidearm",
        EquipmentSlot::Sidearm,
        HitscanWeaponTuning::default(),
        ammo_id,
        0.5,
    )
    .expect("test sidearm");
    let medkit = ItemDefinition::consumable(
        TEST_MEDKIT_NAME,
        "Test Medkit",
        4,
        0.25,
        ItemUseEffect::Heal { amount: 25.0 },
    )
    .expect("test medkit");

    let mut catalog = ItemCatalog::default();
    catalog.register(ammo).expect("register ammo");
    catalog.register(primary).expect("register primary");
    catalog.register(sidearm).expect("register sidearm");
    catalog.register(medkit).expect("register medkit");
    world.insert_resource(catalog);

    let mut loadout = InventoryLoadout::new(TEST_LOADOUT_NAME).expect("test loadout");
    loadout.entries = vec![
        InventoryLoadoutEntry {
            item: item_id(TEST_PRIMARY_NAME),
            quantity: 1,
            equip: true,
        },
        InventoryLoadoutEntry {
            item: item_id(TEST_SIDEARM_NAME),
            quantity: 1,
            equip: true,
        },
        InventoryLoadoutEntry {
            item: item_id(TEST_AMMO_NAME),
            quantity: 60,
            equip: false,
        },
        InventoryLoadoutEntry {
            item: item_id(TEST_MEDKIT_NAME),
            quantity: 1,
            equip: false,
        },
    ];
    let mut loadouts = InventoryLoadoutCatalog::default();
    loadouts.register(loadout).expect("register loadout");
    world.insert_resource(loadouts);
}

fn spawn_test_player(world: &mut World, name: &str) -> newengine_ecs::EntityId {
    install_test_content(world);
    let player = spawn_default_player(world, None, name, Vec3::ZERO);
    apply_loadout(world, player, item_id(TEST_LOADOUT_NAME)).expect("apply test loadout");
    player
}

#[test]
fn f1_visibility_toggle_hides_entire_hud_and_closes_inventory() {
    let mut world = World::new();
    let player = spawn_test_player(&mut world, "hud-visibility-player");
    if let Some(state) = world.resource_mut::<InventoryHudState>() {
        state.open = true;
    }
    if let Some(commands) = world.get_mut::<PlayerCommandFrame>(player) {
        commands
            .actions
            .pressed
            .push(fps_action::HUD_VISIBILITY_TOGGLE.into());
    }

    step_inventory_commands(&mut world, 1);

    let hidden = world
        .resource::<InventoryHudState>()
        .expect("inventory HUD");
    assert!(!hidden.visible);
    assert!(!hidden.open);

    if let Some(commands) = world.get_mut::<PlayerCommandFrame>(player) {
        commands.actions.clear_pulses();
        commands
            .actions
            .pressed
            .push(fps_action::HUD_VISIBILITY_TOGGLE.into());
    }
    step_inventory_commands(&mut world, 2);
    assert!(inventory_hud_is_visible(&world));
}

#[test]
fn command_toggle_and_hotbar_selection_update_inventory_hud() {
    let mut world = World::new();
    let player = spawn_test_player(&mut world, "hud-player");
    if let Some(commands) = world.get_mut::<PlayerCommandFrame>(player) {
        commands
            .actions
            .pressed
            .push(fps_action::INVENTORY_TOGGLE.into());
        commands
            .actions
            .pressed
            .push(fps_action::EQUIP_SIDEARM.into());
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
    let player = spawn_test_player(&mut world, "drag-player");
    let primary = world
        .get::<PlayerInventory>(player)
        .expect("inventory")
        .entries
        .iter()
        .find(|entry| entry.item == item_id(TEST_PRIMARY_NAME))
        .expect("primary")
        .instance_id;
    let initial = world
        .get::<PlayerInventory>(player)
        .expect("inventory")
        .entries
        .iter()
        .position(|entry| entry.instance_id == primary)
        .expect("position");
    reorder_inventory(&mut world, player, primary, 5);
    let moved = world
        .get::<PlayerInventory>(player)
        .expect("inventory")
        .entries
        .iter()
        .position(|entry| entry.instance_id == primary)
        .expect("position");
    assert_ne!(initial, moved);
    equip_dragged_instance(&mut world, player, primary, EquipmentSlot::Primary);
    assert_eq!(
        world
            .get::<PlayerInventory>(player)
            .expect("inventory")
            .equipped_instance(EquipmentSlot::Primary),
        Some(primary)
    );
}

#[test]
fn double_click_consumable_uses_item_and_drop_creates_world_pickup() {
    let mut world = World::new();
    let player = spawn_test_player(&mut world, "use-player");
    let medkit = item_id(TEST_MEDKIT_NAME);
    let _ = give_item(&mut world, player, medkit, 1).expect("give medkit");
    if let Some(health) = world.get_mut::<newengine_engine_runtime::gameplay::Health>(player) {
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
            .get::<newengine_engine_runtime::gameplay::Health>(player)
            .unwrap()
            .current
            > 10.0
    );
    let primary_instance = world
        .get::<PlayerInventory>(player)
        .unwrap()
        .entries
        .iter()
        .find(|entry| entry.item == item_id(TEST_PRIMARY_NAME))
        .unwrap()
        .instance_id;
    drop_instance_quantity(&mut world, player, primary_instance, 1);
    assert!(world
        .query::<newengine_engine_runtime::gameplay::ItemPickup>()
        .next()
        .is_some());
}
