#![forbid(unsafe_op_in_unsafe_fn)]

mod combat;
mod components;
mod fps_demo;
mod inventory;
mod inventory_hud;
mod item_assets;
mod listeners;
mod physics;
mod player;
mod schedule;
mod snapshot;

pub(crate) use combat::{collect_combat_queries, resolve_combat_queries};
pub use combat::{
    drain_interaction_events, drain_weapon_events, Health, HitscanWeaponTuning, Interactable,
    InteractionEvent, InteractionEventBus, PlayerInteractionTuning, PlayerWeaponState, WeaponEvent,
    WeaponEventBus, WeaponEventKind,
};
pub use components::{
    attach_scene_element_core, attach_scene_object_core, scene_entity_by_role, CollisionShapeDesc,
    DisplayMode, DisplayVisibility, FpsDemoGoal, FpsDemoHazard, FpsDemoPickup, FpsDemoRules,
    FpsDemoState, FpsDemoTarget, FpsPlayerTuning, GameReadyWorldLaunchGate,
    GameReadyWorldLaunchGatePhase, GameRunMode, GameplayActor, PhysicsBodyDesc, PhysicsSurface,
    PlayerActor, PlayerCommandFrame, PlayerController, PlayerControllerKind, PlayerEvent,
    PlayerEventBus, PlayerEventKind, PlayerGroundState, PlayerLocomotionState, PlayerModelBinding,
    PlayerStanceKind, PlayerStanceState, PlayerViewVisibility, PlayerViewVisibilityPolicy,
    PlayerVisualKind, PlayerVisualPart, SceneAnchorFollow, SceneEntityAnchor, SceneEntityRole,
    StaticMeshCollider,
};
pub use fps_demo::step_fps_demo_gameplay;
pub use inventory::{
    apply_loadout, default_fps_loadout_id, default_medkit_item_id, default_rifle_ammo_id,
    default_rifle_item_id, drain_inventory_events, drop_item, ensure_default_item_catalog,
    ensure_player_inventory, equip_first_item, equip_item_instance, equipped_reserve_ammo,
    give_default_fps_loadout, give_item, inventory_quantity, persist_equipped_weapon_state,
    remove_item, select_equipment_slot, spawn_item_pickup, spawn_persistent_item_pickup,
    step_world_items, sync_equipped_weapon_runtime, unequip_slot, use_item, EquipmentSlot,
    EquippedWeaponBinding, InventoryEntry, InventoryEvent, InventoryEventBus, InventoryEventKind,
    InventoryLoadout, InventoryLoadoutCatalog, InventoryLoadoutEntry, InventoryMutation,
    ItemCatalog, ItemDefinition, ItemId, ItemInstanceId, ItemKind, ItemPickup, ItemUseEffect,
    PlayerInventory, WeaponItemDefinition, WorldItemDefinition, WorldItemPresentation,
    WorldItemRuntime, WorldItemVisualPart, DEFAULT_FPS_LOADOUT_NAME, DEFAULT_MEDKIT_ITEM_NAME,
    DEFAULT_RIFLE_AMMO_NAME, DEFAULT_RIFLE_ITEM_NAME,
};
pub use inventory_hud::{
    apply_inventory_ui_actions, ensure_inventory_hud_state, inventory_hud_is_open,
    inventory_hud_is_visible, publish_inventory_hud_state, step_inventory_commands,
    InventoryDragState, InventoryHudState,
};
pub use item_assets::{
    compile_authored_item_package, compiled_embedded_fps_item_package,
    decode_authored_item_package, decode_authored_item_package_nef8,
    encode_authored_item_package_nef8, install_compiled_item_package,
    parse_authored_item_package_json, AuthoredItemDefinition, AuthoredItemPackage,
    AuthoredLoadoutDefinition, AuthoredLoadoutEntry, AuthoredUseEffect, AuthoredWeaponDefinition,
    CompiledItemPackage, AUTHORED_ITEM_PACKAGE_SCHEMA, AUTHORED_ITEM_PACKAGE_VERSION,
    NEITEMS_LOGICAL_PATH,
};
pub use listeners::{drain_player_events, emit_player_event, sync_player_view_listeners};
pub use physics::{PhysicsRuntimeFrameIndex, PhysicsSyncModule};
pub use player::{
    apply_player_command_frame, apply_player_fixed_commands, apply_player_input,
    apply_player_stance_geometry, attach_active_camera_to_player, clear_player_input,
    consume_player_transient_input, detach_active_camera_from_player, display_visible_in_mode,
    ensure_physics_body, first_player, is_player_controller_enabled, remove_physics_body,
    spawn_default_player, spawn_default_player_with_tuning, spawn_player_controller_with_tuning,
    update_player_stance_camera,
};
pub use schedule::{
    default_sim_schedule, run_schedule, run_schedule_with_physics_mode,
    run_schedule_with_physics_mode_and_telemetry,
    run_schedule_with_physics_mode_and_telemetry_for_frame, PhysicsIntegrationMode,
};
pub use snapshot::{
    capture_runtime_world_snapshot, restore_runtime_world_snapshot, RuntimeEntitySnapshot,
    RuntimeWorldSnapshot,
};
