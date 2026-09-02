use super::*;
use newengine_engine_runtime::gameplay::{
    apply_loadout, give_item, spawn_default_player, EquipmentSlot, HitscanWeaponTuning,
    InventoryLoadout, InventoryLoadoutCatalog, InventoryLoadoutEntry, ItemCatalog, ItemDefinition,
    ItemId, ItemKind, ItemUseEffect, WeaponFireMode,
};
use newengine_gameplay_fps_api::{
    FpsCharacterMenuPolicySnapshot, FpsGameplayPolicySnapshot, FpsPlayableCharacterPolicy,
};
use newengine_math::Vec3;

const TEST_AMMO_NAME: &str = "test.ammo.primary";
const TEST_PRIMARY_NAME: &str = "test.weapon.primary";
const TEST_SIDEARM_NAME: &str = "test.weapon.sidearm";
const TEST_MEDKIT_NAME: &str = "test.consumable.medkit";
const TEST_LOADOUT_NAME: &str = "test.loadout.hud";
const TEST_CHARACTER_A: &str = "test.character.alpha";
const TEST_CHARACTER_B: &str = "test.character.beta";

fn test_character(id: &str, model: &str) -> FpsPlayableCharacterPolicy {
    FpsPlayableCharacterPolicy {
        id: id.to_owned(),
        family: "Test".to_owned(),
        display_name: id.to_owned(),
        runtime_ready: true,
        runtime_model_ref: Some(model.to_owned()),
        target_height: 1.75,
        hide_in_first_person: true,
        ..FpsPlayableCharacterPolicy::default()
    }
}

fn install_test_character_policy(world: &mut World) {
    let mut policy = FpsGameplayPolicySnapshot::default();
    policy.characters = vec![
        test_character(TEST_CHARACTER_A, "models/test/alpha.ydd@alpha"),
        test_character(TEST_CHARACTER_B, "models/test/beta.ydd@beta"),
    ];
    world.insert_resource(policy);
    world.insert_resource(FpsCharacterMenuPolicySnapshot {
        toggle_action: fps_action::CHARACTER_SELECT_TOGGLE.to_owned(),
        title: "MODEL".to_owned(),
        ..FpsCharacterMenuPolicySnapshot::default()
    });
}

fn item_id(name: &str) -> ItemId {
    ItemId::from_name(name).expect("valid test item id")
}

fn install_test_content(world: &mut World) {
    install_test_character_policy(world);
    let ammo = ItemDefinition::stackable(TEST_AMMO_NAME, "Test Ammo", ItemKind::Ammo, 240, 0.01)
        .expect("test ammo");
    let ammo_id = ammo.id;
    let primary = ItemDefinition::weapon(
        TEST_PRIMARY_NAME,
        "Test Primary",
        EquipmentSlot::Primary,
        HitscanWeaponTuning::default(),
        ammo_id,
        WeaponFireMode::SemiAuto,
        1.0,
    )
    .expect("test primary");
    let sidearm = ItemDefinition::weapon(
        TEST_SIDEARM_NAME,
        "Test Sidearm",
        EquipmentSlot::Sidearm,
        HitscanWeaponTuning::default(),
        ammo_id,
        WeaponFireMode::SemiAuto,
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
fn character_selector_starts_closed_and_never_requires_focus_state() {
    let mut world = World::new();
    let _player = spawn_test_player(&mut world, "character-selector-default-closed-player");
    ensure_inventory_hud_state(&mut world);

    let state = world
        .resource::<InventoryHudState>()
        .expect("inventory HUD");
    assert!(
        !state.character_select_open,
        "selector must default to closed"
    );

    let output = publish_inventory_hud_state(&mut world, 1);
    let changes = output
        .patches
        .first()
        .expect("initial HUD patch")
        .patch
        .changes
        .as_slice();
    assert!(changes.iter().any(|change| {
        change.source_id == "character"
            && change.path == "open"
            && change.value == serde_json::json!(false)
    }));
    assert!(
        !changes
            .iter()
            .any(|change| { change.source_id == "character" && change.path == "focused" }),
        "selector visibility/repaint must not depend on a synthetic focus property"
    );
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
fn character_menu_weapon_section_selects_available_hotbar_weapon_and_stays_open() {
    let mut world = World::new();
    let player = spawn_test_player(&mut world, "character-menu-weapon-player");
    ensure_inventory_hud_state(&mut world);
    world
        .resource_mut::<InventoryHudState>()
        .expect("inventory HUD")
        .character_select_open = true;

    assert_eq!(
        world
            .get::<PlayerInventory>(player)
            .expect("inventory")
            .active_slot,
        Some(EquipmentSlot::Primary)
    );

    let frame = UiEventDispatchFrame {
        focused_node: Some(newengine_ui_api::UiHitTestResult {
            surface_id: INVENTORY_HUD_SURFACE_ID.to_owned(),
            node_id: "inventory.hotbar.3".to_owned(),
            action_id: Some(INVENTORY_UI_ACTION_HOTBAR.to_owned()),
            ..Default::default()
        }),
        actions: vec![newengine_ui_api::UiActionDispatch {
            surface_id: INVENTORY_HUD_SURFACE_ID.to_owned(),
            node_id: "inventory.hotbar.3".to_owned(),
            action_id: INVENTORY_UI_ACTION_HOTBAR.to_owned(),
            trigger: UiNodeEventTrigger::Click,
            ..Default::default()
        }],
        ..Default::default()
    };

    assert!(FpsInventoryHudProvider.dispatch_actions(&mut world, &frame));
    assert_eq!(
        world
            .get::<PlayerInventory>(player)
            .expect("inventory")
            .active_slot,
        Some(EquipmentSlot::Sidearm),
        "menu Weapon row must select the canonical equipped weapon slot"
    );
    assert!(
        world
            .resource::<InventoryHudState>()
            .expect("inventory HUD")
            .character_select_open,
        "weapon selection must not close the M menu"
    );
    assert_eq!(
        world
            .get::<EquippedWeaponBinding>(player)
            .expect("runtime weapon binding")
            .item,
        item_id(TEST_SIDEARM_NAME),
        "runtime weapon presentation/combat binding must follow menu selection"
    );
}

#[test]
fn published_weapon_section_only_exposes_equipped_available_weapons() {
    let mut world = World::new();
    let _player = spawn_test_player(&mut world, "character-menu-weapon-publish-player");
    let output = publish_inventory_hud_state(&mut world, 1);
    let changes = &output.patches.first().expect("HUD patch").patch.changes;

    let value = |source: &str, path: &str| {
        changes
            .iter()
            .find(|change| change.source_id == source && change.path == path)
            .map(|change| change.value.clone())
            .expect("published weapon field")
    };

    assert_eq!(value("hotbar_1", "visible"), serde_json::json!(true));
    assert_eq!(
        value("hotbar_1", "label"),
        serde_json::json!("1  Test Primary")
    );
    assert_eq!(value("hotbar_3", "visible"), serde_json::json!(true));
    assert_eq!(
        value("hotbar_3", "label"),
        serde_json::json!("3  Test Sidearm")
    );
    assert_eq!(value("hotbar_2", "visible"), serde_json::json!(false));
    assert_eq!(value("hotbar_4", "visible"), serde_json::json!(false));
    assert_eq!(value("hotbar_5", "visible"), serde_json::json!(false));
}

#[test]
fn closed_character_menu_skips_registry_serialization() {
    let mut world = World::new();
    let _player = spawn_test_player(&mut world, "character-menu-closed-registry-player");
    ensure_inventory_hud_state(&mut world);

    let output = publish_inventory_hud_state(&mut world, 1);
    let changes = &output.patches.first().expect("HUD patch").patch.changes;
    assert!(changes.iter().all(|change| {
        !(change.source_id == "character"
            && matches!(
                change.path.as_str(),
                "registered_characters"
                    | "registered_weapons"
                    | "registered_character_count"
                    | "registered_weapon_count"
            ))
    }));
}

#[test]
fn character_menu_publishes_registry_and_starts_on_characters_category() {
    let mut world = World::new();
    let _player = spawn_test_player(&mut world, "character-menu-registry-player");
    ensure_inventory_hud_state(&mut world);
    world
        .resource_mut::<InventoryHudState>()
        .expect("inventory HUD")
        .character_select_open = true;

    let output = publish_inventory_hud_state(&mut world, 1);
    let changes = &output.patches.first().expect("HUD patch").patch.changes;
    let value = |source: &str, path: &str| {
        changes
            .iter()
            .find(|change| change.source_id == source && change.path == path)
            .map(|change| change.value.clone())
            .expect("published character menu field")
    };

    assert_eq!(
        value("character", "category_characters_selected"),
        serde_json::json!(true)
    );
    assert_eq!(
        value("character", "category_weapons_selected"),
        serde_json::json!(false)
    );
    assert_eq!(
        value("character", "characters_visible"),
        serde_json::json!(true)
    );
    assert_eq!(
        value("character", "weapons_visible"),
        serde_json::json!(false)
    );
    assert_eq!(
        value("character", "registered_character_count"),
        serde_json::json!(2)
    );
    let characters = value("character", "registered_characters");
    let characters = characters.as_array().expect("registered character list");
    assert_eq!(characters.len(), 2);
    assert_eq!(
        characters[0]["entity_key"],
        serde_json::json!(TEST_CHARACTER_A)
    );
    assert_eq!(
        characters[1]["entity_key"],
        serde_json::json!(TEST_CHARACTER_B)
    );
}

#[test]
fn character_menu_category_actions_switch_retained_collection_without_closing_menu() {
    let mut world = World::new();
    let _player = spawn_test_player(&mut world, "character-menu-category-player");
    ensure_inventory_hud_state(&mut world);
    world
        .resource_mut::<InventoryHudState>()
        .expect("inventory HUD")
        .character_select_open = true;

    let frame = UiEventDispatchFrame {
        actions: vec![newengine_ui_api::UiActionDispatch {
            surface_id: INVENTORY_HUD_SURFACE_ID.to_owned(),
            node_id: "character.category.weapons".to_owned(),
            action_id: CHARACTER_UI_ACTION_CATEGORY_WEAPONS.to_owned(),
            trigger: UiNodeEventTrigger::Click,
            ..Default::default()
        }],
        ..Default::default()
    };
    assert!(FpsInventoryHudProvider.dispatch_actions(&mut world, &frame));
    let state = world
        .resource::<InventoryHudState>()
        .expect("inventory HUD");
    assert!(state.character_select_open);
    assert_eq!(state.character_category, CharacterMenuCategory::Weapons);

    let output = publish_inventory_hud_state(&mut world, 2);
    let changes = &output.patches.first().expect("HUD patch").patch.changes;
    assert!(changes.iter().any(|change| {
        change.source_id == "character"
            && change.path == "weapons_visible"
            && change.value == serde_json::json!(true)
    }));
}

#[test]
fn m_command_toggles_playable_character_selector_open_and_closed() {
    let mut world = World::new();
    let player = spawn_test_player(&mut world, "character-selector-player");
    if let Some(commands) = world.get_mut::<PlayerCommandFrame>(player) {
        commands.source_frame = 1;
        commands
            .actions
            .held
            .push(fps_action::CHARACTER_SELECT_TOGGLE.into());
        commands
            .actions
            .pressed
            .push(fps_action::CHARACTER_SELECT_TOGGLE.into());
    }

    step_inventory_commands(&mut world, 1);
    assert!(
        world
            .resource::<InventoryHudState>()
            .expect("inventory HUD")
            .character_select_open
    );

    // Physical release immediately re-arms the next intentional M press.
    if let Some(commands) = world.get_mut::<PlayerCommandFrame>(player) {
        commands.source_frame = 2;
        commands.actions.clear_pulses();
        commands
            .actions
            .released
            .push(fps_action::CHARACTER_SELECT_TOGGLE.into());
    }
    step_inventory_commands(&mut world, 2);

    if let Some(commands) = world.get_mut::<PlayerCommandFrame>(player) {
        commands.source_frame = 3;
        commands.actions.clear_pulses();
        commands
            .actions
            .held
            .push(fps_action::CHARACTER_SELECT_TOGGLE.into());
        commands
            .actions
            .pressed
            .push(fps_action::CHARACTER_SELECT_TOGGLE.into());
    }
    step_inventory_commands(&mut world, 3);
    assert!(
        !world
            .resource::<InventoryHudState>()
            .expect("inventory HUD")
            .character_select_open
    );
}

#[test]
fn single_m_press_is_consumed_once_across_multiple_fixed_ticks() {
    let mut world = World::new();
    let player = spawn_test_player(&mut world, "character-selector-fixed-step-player");
    if let Some(commands) = world.get_mut::<PlayerCommandFrame>(player) {
        commands.source_frame = 77;
        commands
            .actions
            .held
            .push(fps_action::CHARACTER_SELECT_TOGGLE.into());
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
fn character_selector_semantic_ui_actions_do_not_double_drive_retained_focus() {
    let mut world = World::new();
    let player = spawn_test_player(&mut world, "character-selector-keyboard-authority-player");

    if let Some(commands) = world.get_mut::<PlayerCommandFrame>(player) {
        commands.source_frame = 1;
        commands
            .actions
            .pressed
            .push(fps_action::CHARACTER_SELECT_TOGGLE.into());
    }
    step_inventory_commands(&mut world, 1);
    assert!(
        world
            .resource::<InventoryHudState>()
            .expect("inventory HUD")
            .character_select_open
    );

    let initial_index = world
        .resource::<InventoryHudState>()
        .expect("inventory HUD")
        .character_nav_index;
    if let Some(commands) = world.get_mut::<PlayerCommandFrame>(player) {
        commands.source_frame = 2;
        commands.actions.clear_pulses();
        commands
            .actions
            .pressed
            .extend([fps_action::UI_NAV_DOWN.into(), fps_action::UI_ACCEPT.into()]);
    }
    step_inventory_commands(&mut world, 2);

    let state = world
        .resource::<InventoryHudState>()
        .expect("inventory HUD");
    assert!(state.character_select_open);
    assert_eq!(
        state.character_nav_index, initial_index,
        "gameplay command path must not compete with Aurelia retained keyboard focus"
    );
    assert!(world.get::<PlayableCharacterSelection>(player).is_none());
    assert!(!fps_noclip_enabled(&world, player));
}

#[test]
fn character_selector_retained_focus_updates_presented_nav_index() {
    let mut world = World::new();
    let _player = spawn_test_player(&mut world, "character-selector-focus-sync-player");
    ensure_inventory_hud_state(&mut world);
    world
        .resource_mut::<InventoryHudState>()
        .expect("inventory HUD")
        .character_select_open = true;

    let second = playable_character_variants(&world)
        .get(1)
        .expect("second playable character")
        .id
        .clone();
    let frame = UiEventDispatchFrame {
        focused_node: Some(newengine_ui_api::UiHitTestResult {
            surface_id: INVENTORY_HUD_SURFACE_ID.to_owned(),
            node_id: "character.keyboard.focus".to_owned(),
            action_id: Some(format!("game.character.select.{second}")),
            ..Default::default()
        }),
        ..Default::default()
    };

    FpsInventoryHudProvider.dispatch_actions(&mut world, &frame);
    assert_eq!(
        world
            .resource::<InventoryHudState>()
            .expect("inventory HUD")
            .character_nav_index,
        1
    );
}

#[test]
fn character_selector_space_click_toggles_noclip_once_and_keeps_menu_open() {
    let mut world = World::new();
    let player = spawn_test_player(&mut world, "character-selector-noclip-ui-player");
    ensure_inventory_hud_state(&mut world);
    world
        .resource_mut::<InventoryHudState>()
        .expect("inventory HUD")
        .character_select_open = true;

    let frame = UiEventDispatchFrame {
        focused_node: Some(newengine_ui_api::UiHitTestResult {
            surface_id: INVENTORY_HUD_SURFACE_ID.to_owned(),
            node_id: "character.noclip".to_owned(),
            action_id: Some(CHARACTER_UI_ACTION_NOCLIP_TOGGLE.to_owned()),
            ..Default::default()
        }),
        actions: vec![newengine_ui_api::UiActionDispatch {
            surface_id: INVENTORY_HUD_SURFACE_ID.to_owned(),
            node_id: "character.noclip".to_owned(),
            action_id: CHARACTER_UI_ACTION_NOCLIP_TOGGLE.to_owned(),
            trigger: UiNodeEventTrigger::Click,
            ..Default::default()
        }],
        ..Default::default()
    };

    assert!(FpsInventoryHudProvider.dispatch_actions(&mut world, &frame));
    assert!(fps_noclip_enabled(&world, player));
    assert!(
        world
            .resource::<InventoryHudState>()
            .expect("inventory HUD")
            .character_select_open
    );
    assert_eq!(
        world
            .resource::<InventoryHudState>()
            .expect("inventory HUD")
            .character_nav_index,
        playable_character_variants(&world).len()
    );
}

#[test]
fn m_release_does_not_retrigger_stale_press_when_no_fixed_tick_cleared_pulses() {
    let mut world = World::new();
    let player = spawn_test_player(&mut world, "character-selector-render-frame-release-player");

    if let Some(commands) = world.get_mut::<PlayerCommandFrame>(player) {
        commands.source_frame = 1;
        commands
            .actions
            .held
            .push(fps_action::CHARACTER_SELECT_TOGGLE.into());
        commands
            .actions
            .pressed
            .push(fps_action::CHARACTER_SELECT_TOGGLE.into());
    }
    step_inventory_commands(&mut world, 0);
    assert!(
        world
            .resource::<InventoryHudState>()
            .expect("inventory HUD")
            .character_select_open
    );

    // Model a render-cadence release before any fixed simulation step consumed the
    // previous pressed pulse: both stale `pressed` and fresh `released` coexist.
    if let Some(commands) = world.get_mut::<PlayerCommandFrame>(player) {
        commands.source_frame = 2;
        commands.actions.held.clear();
        commands
            .actions
            .released
            .push(fps_action::CHARACTER_SELECT_TOGGLE.into());
    }
    step_inventory_commands(&mut world, 0);

    let state = world
        .resource::<InventoryHudState>()
        .expect("inventory HUD");
    assert!(
        state.character_select_open,
        "key release must not replay the stale M press"
    );
    assert!(
        !state.character_toggle_latched,
        "release must still re-arm the next physical M press"
    );
}

#[test]
fn m_release_rearms_next_toggle_immediately() {
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

    if let Some(commands) = world.get_mut::<PlayerCommandFrame>(player) {
        commands.source_frame = 2;
        commands.actions.clear_pulses();
        commands
            .actions
            .pressed
            .push(fps_action::CHARACTER_SELECT_TOGGLE.into());
    }
    step_inventory_commands(&mut world, 2);
    assert!(
        world
            .resource::<InventoryHudState>()
            .expect("inventory HUD")
            .character_select_open
    );

    if let Some(commands) = world.get_mut::<PlayerCommandFrame>(player) {
        commands.source_frame = 3;
        commands.actions.clear_pulses();
        commands
            .actions
            .released
            .push(fps_action::CHARACTER_SELECT_TOGGLE.into());
    }
    step_inventory_commands(&mut world, 3);
    assert!(
        !world
            .resource::<InventoryHudState>()
            .expect("inventory HUD")
            .character_toggle_latched
    );

    if let Some(commands) = world.get_mut::<PlayerCommandFrame>(player) {
        commands.source_frame = 4;
        commands.actions.clear_pulses();
        commands
            .actions
            .pressed
            .push(fps_action::CHARACTER_SELECT_TOGGLE.into());
    }
    step_inventory_commands(&mut world, 4);
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
        capture.keyboard,
        "character selector must own keyboard focus while raw sampling keeps M available"
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

    let variant = character_variants::variant_by_id(&world, TEST_CHARACTER_B)
        .expect("project-authored alternate character")
        .clone();
    assert!(select_playable_character(&mut world, player, &variant));

    assert!(
        !world
            .resource::<InventoryHudState>()
            .expect("inventory HUD")
            .character_select_open
    );
    assert_eq!(
        selected_variant(&world, player).map(|variant| variant.id.as_str()),
        Some(TEST_CHARACTER_B)
    );
    let assignment = world
        .get::<newengine_engine_runtime::gameplay::PlayerModelAssignment>(player)
        .expect("character selection must publish a model assignment");
    assert_eq!(assignment.source, "models/test/beta.ydd@beta");
    assert!(assignment.revision > 0);
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
            node_id: "character.option.test.beta".to_owned(),
            action_id: format!("game.character.select.{TEST_CHARACTER_B}"),
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
        selected_variant(&world, player).map(|variant| variant.id.as_str()),
        Some(TEST_CHARACTER_B)
    );
    let assignment = world
        .get::<newengine_engine_runtime::gameplay::PlayerModelAssignment>(player)
        .expect("UI dispatch must publish a model assignment");
    assert_eq!(assignment.source, "models/test/beta.ydd@beta");
    assert!(assignment.revision > 0);
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

#[test]
fn hud_publishes_read_only_character_vitals_and_republishes_on_vital_changes() {
    let mut world = World::new();
    let player = spawn_test_player(&mut world, "vitals-hud-player");
    world
        .get_mut::<newengine_engine_runtime::gameplay::Health>(player)
        .unwrap()
        .current = 42.0;
    {
        let stamina = world
            .get_mut::<newengine_engine_runtime::gameplay::Stamina>(player)
            .unwrap();
        stamina.current = 11.0;
        stamina.exhausted = true;
    }
    let _ = world.insert(
        player,
        newengine_engine_runtime::gameplay::CharacterInjuryState {
            injured: true,
            revision: 1,
        },
    );
    let _ = world.insert(
        player,
        newengine_engine_runtime::gameplay::CharacterHitReactionState {
            kind: newengine_engine_runtime::gameplay::CharacterHitReactionKind::Flinch,
            remaining_seconds: 0.1,
            sequence: 1,
            source: 2,
            hit_zone: Some("torso".to_owned()),
            point: Vec3::ZERO,
            impulse: Vec3::ZERO,
            applied_damage: 5.0,
            health_fraction: 0.42,
            revision: 1,
        },
    );

    let output = publish_inventory_hud_state(&mut world, 1);
    let changes = &output.patches.first().expect("HUD patch").patch.changes;
    let value = |path: &str| {
        changes
            .iter()
            .find(|change| change.source_id == "player" && change.path == path)
            .map(|change| change.value.clone())
            .expect("published vitals field")
    };
    let health_normalized = value("health_normalized")
        .as_f64()
        .expect("health_normalized number");
    assert!((health_normalized - 0.42).abs() < 1.0e-5);
    assert_eq!(value("health_label"), serde_json::json!("42 / 100"));
    let stamina_normalized = value("stamina_normalized")
        .as_f64()
        .expect("stamina_normalized number");
    assert!((stamina_normalized - 0.11).abs() < 1.0e-5);
    assert_eq!(value("stamina_label"), serde_json::json!("11 / 100"));
    assert_eq!(value("stamina_exhausted"), serde_json::json!(true));
    assert_eq!(value("injured"), serde_json::json!(true));
    assert_eq!(value("damage_flash"), serde_json::json!(true));
    assert_eq!(value("hit_reaction"), serde_json::json!("flinch"));
    assert_eq!(value("dead"), serde_json::json!(false));

    world
        .get_mut::<newengine_engine_runtime::gameplay::Health>(player)
        .unwrap()
        .current = 41.0;
    let changed = publish_inventory_hud_state(&mut world, 2);
    assert_eq!(
        changed.patches.len(),
        1,
        "health mutation must invalidate HUD fingerprint"
    );

    let _ = world.insert(
        player,
        newengine_engine_runtime::gameplay::CharacterLifeState::Dead,
    );
    let dead = publish_inventory_hud_state(&mut world, 3);
    let changes = &dead.patches.first().expect("dead HUD patch").patch.changes;
    assert_eq!(
        changes
            .iter()
            .find(|change| change.source_id == "player" && change.path == "dead")
            .map(|change| change.value.clone()),
        Some(serde_json::json!(true))
    );
}
