#![forbid(unsafe_op_in_unsafe_fn)]

mod combat;
mod components;
mod content;
mod execution;
mod inventory;
mod listeners;
mod physics;
mod physics_queries;
mod player;
mod schedule;
mod snapshot;
mod ui;

pub use combat::{
    drain_interaction_events, drain_weapon_events, Health, HitscanWeaponTuning, Interactable,
    InteractionEvent, InteractionEventBus, PendingHitscan, PendingInteraction,
    PlayerInteractionTuning, PlayerWeaponState, WeaponEvent, WeaponEventBus, WeaponEventKind,
};
pub use components::{
    attach_scene_element_core, attach_scene_object_core, scene_entity_by_role, CharacterBody,
    CharacterMotionTuning, CloudShadowRenderState, CollisionShapeDesc, DisplayMode,
    DisplayVisibility, EnvironmentDomeRenderState, EnvironmentPostFxState, GameRunMode,
    GameplayActor, PhysicsBodyDesc, PhysicsSurface, PhysicsWorldSettings, PlayerActor,
    PlayerCommandFrame, PlayerController, PlayerControllerKind, PlayerEvent, PlayerEventBus,
    PlayerEventKind, PlayerGroundState, PlayerLocomotionState, PlayerModelBinding,
    PlayerStanceKind, PlayerStanceState, PlayerViewVisibility, PlayerViewVisibilityPolicy,
    PlayerVisualKind, PlayerVisualPart, PreparedRenderMesh, ResidencyProgress, SceneAnchorFollow,
    SceneEntityAnchor, SceneEntityRole, StaticMeshCollider, TerrainMaterialLayers,
    WorldActivationPhase, WorldActivationState, WorldAssemblyProgress, WorldClearColor,
};
pub use content::{GameplayContentProvider, GameplayContentProviderRegistry};
pub use execution::{
    GameplayExecutionPhase, GameplayFrame, GameplaySystemProvider, GameplaySystemProviderRegistry,
    GameplayWorld,
};
pub use inventory::{
    apply_loadout, consume_equipped_ammo, drain_inventory_events, drop_item,
    ensure_inventory_runtime, ensure_player_inventory, equip_first_item, equip_item_instance,
    equipped_reserve_ammo, give_item, inventory_quantity, persist_equipped_weapon_state,
    remove_item, select_equipment_slot, spawn_item_pickup, spawn_persistent_item_pickup,
    step_world_items, sync_equipped_weapon_runtime, try_collect_item_pickup, unequip_slot,
    use_item, EquipmentSlot, EquippedWeaponBinding, InventoryEntry, InventoryEvent,
    InventoryEventBus, InventoryEventKind, InventoryLoadout, InventoryLoadoutCatalog,
    InventoryLoadoutEntry, InventoryMutation, ItemCatalog, ItemDefinition, ItemId, ItemInstanceId,
    ItemKind, ItemPickup, ItemUseEffect, PlayerInventory, WeaponItemDefinition,
    WorldItemDefinition, WorldItemPresentation, WorldItemRuntime, WorldItemVisualPart,
};
pub use listeners::{drain_player_events, emit_player_event, sync_player_view_listeners};
pub use physics::{PhysicsRuntimeFrameIndex, PhysicsSyncModule};
pub use physics_queries::{GameplayPhysicsQueryProvider, GameplayPhysicsQueryProviderRegistry};
pub use player::{
    apply_player_command_frame, apply_player_input, apply_player_stance_geometry,
    attach_active_camera_to_player, clear_player_input, consume_player_transient_input,
    detach_active_camera_from_player, display_visible_in_mode, ensure_physics_body, first_player,
    is_player_controller_enabled, remove_physics_body, spawn_default_player,
    spawn_player_controller, update_player_stance_camera,
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
pub use ui::{
    gameplay_input_capture, gameplay_modal_state, GameplayInputCapture, GameplayModalState,
    GameplayUiFrameOutput, GameplayUiProvider, GameplayUiProviderRegistry, GameplayUiStatePatch,
    GameplayUiSurfaceVisibility,
};
