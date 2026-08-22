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
        commands.source_frame = 1;
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
        commands.source_frame = 2;
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
fn hud_surface_visibility_is_published_only_on_visibility_edges() {
    let mut world = World::new();
    let _player = spawn_test_player(&mut world, "hud-visibility-edge-player");

    let first = publish_inventory_hud_state(&mut world, 1);
    assert_eq!(first.surface_visibility.len(), 1);
    assert_eq!(
        first.surface_visibility[0].surface_id,
        INVENTORY_HUD_SURFACE_ID
    );
    assert!(first.surface_visibility[0].visible);

    world
        .resource_mut::<InventoryHudState>()
        .expect("inventory HUD")
        .toggle_inventory();
    let content_only = publish_inventory_hud_state(&mut world, 2);
    assert!(
        content_only.surface_visibility.is_empty(),
        "ordinary HUD state changes must not re-assert visible=true every frame"
    );

    world
        .resource_mut::<InventoryHudState>()
        .expect("inventory HUD")
        .toggle_visibility();
    let hidden = publish_inventory_hud_state(&mut world, 3);
    assert_eq!(hidden.surface_visibility.len(), 1);
    assert!(!hidden.surface_visibility[0].visible);

    let stable_hidden = publish_inventory_hud_state(&mut world, 4);
    assert!(stable_hidden.surface_visibility.is_empty());
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
fn m_command_toggles_playable_character_selector_open_and_closed() {
    let mut world = World::new();
    let player = spawn_test_player(&mut world, "character-selector-player");
    if let Some(commands) = world.get_mut::<PlayerCommandFrame>(player) {
        commands.source_frame = 1;
        commands
            .actions
            .pressed
            .push(fps_action::CHARACTER_SELECT_TOGGLE.into());
    }

    step_inventory_commands(&mut world, 1);
    {
        let state = world
            .resource::<InventoryHudState>()
            .expect("inventory HUD");
        assert!(state.character_select_open);
        assert!(!state.open);
    }

    // Release the logical M edge long enough to re-arm after provider bounce noise.
    for source_frame in 2..=91 {
        if let Some(commands) = world.get_mut::<PlayerCommandFrame>(player) {
            commands.source_frame = source_frame;
            commands.actions.clear_pulses();
        }
        step_inventory_commands(&mut world, source_frame);
    }

    if let Some(commands) = world.get_mut::<PlayerCommandFrame>(player) {
        commands.source_frame = 92;
        commands
            .actions
            .pressed
            .push(fps_action::CHARACTER_SELECT_TOGGLE.into());
    }
    step_inventory_commands(&mut world, 92);

    let state = world
        .resource::<InventoryHudState>()
        .expect("inventory HUD");
    assert!(!state.character_select_open);
}

#[test]
fn single_m_press_is_consumed_once_across_multiple_fixed_ticks() {
    let mut world = World::new();
    let player = spawn_test_player(&mut world, "character-selector-fixed-step-player");
    if let Some(commands) = world.get_mut::<PlayerCommandFrame>(player) {
        commands.source_frame = 77;
        commands
            .actions
            .pressed
            .push(fps_action::CHARACTER_SELECT_TOGGLE.into());
    }

    step_inventory_commands(&mut world, 1001);
    step_inventory_commands(&mut world, 1002);
    step_inventory_commands(&mut world, 1003);

    let state = world
        .resource::<InventoryHudState>()
        .expect("inventory HUD");
    assert!(
        state.character_select_open,
        "one sampled M press must toggle exactly once even across several fixed ticks"
    );
    assert_eq!(state.last_consumed_pulse_source_frame, Some(77));
}

#[test]
fn character_selector_arrow_navigation_and_enter_selects_focused_variant() {
    let mut world = World::new();
    let player = spawn_test_player(&mut world, "character-selector-keyboard-player");

    if let Some(commands) = world.get_mut::<PlayerCommandFrame>(player) {
        commands.source_frame = 1;
        commands
            .actions
            .pressed
            .push(fps_action::CHARACTER_SELECT_TOGGLE.into());
    }
    step_inventory_commands(&mut world, 1);
    assert_eq!(
        world
            .resource::<InventoryHudState>()
            .expect("inventory HUD")
            .character_nav_index,
        0
    );

    if let Some(commands) = world.get_mut::<PlayerCommandFrame>(player) {
        commands.source_frame = 2;
        commands.actions.clear_pulses();
        commands
            .actions
            .pressed
            .push(fps_action::UI_NAV_DOWN.into());
    }
    step_inventory_commands(&mut world, 2);
    assert_eq!(
        world
            .resource::<InventoryHudState>()
            .expect("inventory HUD")
            .character_nav_index,
        1
    );

    if let Some(commands) = world.get_mut::<PlayerCommandFrame>(player) {
        commands.source_frame = 3;
        commands.actions.clear_pulses();
        commands.actions.pressed.push(fps_action::UI_ACCEPT.into());
    }
    step_inventory_commands(&mut world, 3);

    let state = world
        .resource::<InventoryHudState>()
        .expect("inventory HUD");
    assert!(!state.character_select_open);
    assert_eq!(
        selected_variant(&world, player).map(|variant| variant.id),
        Some(character_variants::ABBY_SEATTLE_709_ID)
    );
}

#[test]
fn character_selector_arrow_up_wraps_and_escape_closes_without_selection() {
    let mut world = World::new();
    let player = spawn_test_player(&mut world, "character-selector-wrap-player");

    if let Some(commands) = world.get_mut::<PlayerCommandFrame>(player) {
        commands.source_frame = 10;
        commands
            .actions
            .pressed
            .push(fps_action::CHARACTER_SELECT_TOGGLE.into());
    }
    step_inventory_commands(&mut world, 10);

    if let Some(commands) = world.get_mut::<PlayerCommandFrame>(player) {
        commands.source_frame = 11;
        commands.actions.clear_pulses();
        commands.actions.pressed.push(fps_action::UI_NAV_UP.into());
    }
    step_inventory_commands(&mut world, 11);
    assert_eq!(
        world
            .resource::<InventoryHudState>()
            .expect("inventory HUD")
            .character_nav_index,
        PLAYABLE_CHARACTER_VARIANTS.len() - 1
    );

    if let Some(commands) = world.get_mut::<PlayerCommandFrame>(player) {
        commands.source_frame = 12;
        commands.actions.clear_pulses();
        commands.actions.pressed.push(fps_action::UI_BACK.into());
    }
    step_inventory_commands(&mut world, 12);

    assert!(
        !world
            .resource::<InventoryHudState>()
            .expect("inventory HUD")
            .character_select_open
    );
    assert!(world.get::<PlayableCharacterSelection>(player).is_none());
}

#[test]
fn repeated_pressed_pulse_across_source_frames_toggles_character_menu_once() {
    let mut world = World::new();
    let player = spawn_test_player(&mut world, "character-selector-debounce-player");

    if let Some(commands) = world.get_mut::<PlayerCommandFrame>(player) {
        commands.source_frame = 1;
        commands
            .actions
            .pressed
            .push(fps_action::CHARACTER_SELECT_TOGGLE.into());
    }
    step_inventory_commands(&mut world, 1);

    // Provider bounce: short clean gaps followed by duplicate M press/release pairs.
    let mut frame = 2u64;
    for _burst in 0..3 {
        for _ in 0..30 {
            if let Some(commands) = world.get_mut::<PlayerCommandFrame>(player) {
                commands.source_frame = frame;
                commands.actions.clear_pulses();
            }
            step_inventory_commands(&mut world, frame);
            frame += 1;
        }
        if let Some(commands) = world.get_mut::<PlayerCommandFrame>(player) {
            commands.source_frame = frame;
            commands.actions.clear_pulses();
            commands
                .actions
                .pressed
                .push(fps_action::CHARACTER_SELECT_TOGGLE.into());
        }
        step_inventory_commands(&mut world, frame);
        frame += 1;
    }

    let state = world
        .resource::<InventoryHudState>()
        .expect("inventory HUD");
    assert!(
        state.character_select_open,
        "duplicate M burst must not blink the menu"
    );
    assert!(state.character_toggle_latched);

    // A stable release window re-arms the next intentional M press.
    for _ in 0..90 {
        if let Some(commands) = world.get_mut::<PlayerCommandFrame>(player) {
            commands.source_frame = frame;
            commands.actions.clear_pulses();
        }
        step_inventory_commands(&mut world, frame);
        frame += 1;
    }
    assert!(
        !world
            .resource::<InventoryHudState>()
            .expect("inventory HUD")
            .character_toggle_latched
    );

    if let Some(commands) = world.get_mut::<PlayerCommandFrame>(player) {
        commands.source_frame = frame;
        commands
            .actions
            .pressed
            .push(fps_action::CHARACTER_SELECT_TOGGLE.into());
    }
    step_inventory_commands(&mut world, frame);
    assert!(
        !world
            .resource::<InventoryHudState>()
            .expect("inventory HUD")
            .character_select_open
    );
}

#[test]
fn character_selector_capture_keeps_m_keyboard_path_alive() {
    let mut world = World::new();
    let _player = spawn_test_player(&mut world, "character-selector-capture-player");
    ensure_inventory_hud_state(&mut world);
    let state = world
        .resource_mut::<InventoryHudState>()
        .expect("inventory HUD");
    state.character_select_open = true;

    let capture = FpsInventoryHudProvider.input_capture(&world);
    assert!(capture.pointer);
    assert!(
        !capture.keyboard,
        "keyboard must remain sampled so M can close the selector"
    );
    assert!(capture.block_gameplay_actions);
    assert!(capture.block_camera_navigation);
    assert!(capture.block_player_movement);
    assert!(capture.release_cursor);
}

#[test]
fn selecting_another_playable_character_closes_selector() {
    let mut world = World::new();
    let player = spawn_test_player(&mut world, "character-switch-player");
    ensure_inventory_hud_state(&mut world);
    world
        .resource_mut::<InventoryHudState>()
        .expect("inventory HUD")
        .character_select_open = true;

    let variant = character_variants::variant_by_id(character_variants::ABIGAIL_LEGACY_ID)
        .expect("legacy alternate character");
    assert!(select_playable_character(&mut world, player, variant));

    assert!(
        !world
            .resource::<InventoryHudState>()
            .expect("inventory HUD")
            .character_select_open
    );
    assert_eq!(
        selected_variant(&world, player).map(|variant| variant.id),
        Some(character_variants::ABIGAIL_LEGACY_ID)
    );
}

#[test]
fn dropdown_character_selection_dispatch_closes_menu() {
    let mut world = World::new();
    let player = spawn_test_player(&mut world, "character-dropdown-player");
    ensure_inventory_hud_state(&mut world);
    world
        .resource_mut::<InventoryHudState>()
        .expect("inventory HUD")
        .character_select_open = true;

    if let Some(commands) = world.get_mut::<PlayerCommandFrame>(player) {
        commands
            .actions
            .held
            .push(fps_action::PLAYER_FIRE_PRIMARY.into());
        commands.actions.held.push(fps_action::PLAYER_AIM.into());
        commands
            .actions
            .pressed
            .push(fps_action::PLAYER_LAUNCH_PROJECTILE.into());
    }

    let frame = UiEventDispatchFrame {
        actions: vec![newengine_ui_api::UiActionDispatch {
            surface_id: INVENTORY_HUD_SURFACE_ID.to_owned(),
            node_id: "character.option.abigail.legacy".to_owned(),
            action_id: "game.character.select.abigail_legacy".to_owned(),
            trigger: UiNodeEventTrigger::Click,
            ..Default::default()
        }],
        ..Default::default()
    };

    assert!(FpsInventoryHudProvider.dispatch_actions(&mut world, &frame));
    assert!(
        !world
            .resource::<InventoryHudState>()
            .expect("inventory HUD")
            .character_select_open
    );
    assert_eq!(
        selected_variant(&world, player).map(|variant| variant.id),
        Some(character_variants::ABIGAIL_LEGACY_ID)
    );
    let commands = world
        .get::<PlayerCommandFrame>(player)
        .expect("player commands");
    assert!(!commands.actions.is_held(fps_action::PLAYER_FIRE_PRIMARY));
    assert!(!commands.actions.is_held(fps_action::PLAYER_AIM));
    assert!(!commands
        .actions
        .is_pressed(fps_action::PLAYER_LAUNCH_PROJECTILE));
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
