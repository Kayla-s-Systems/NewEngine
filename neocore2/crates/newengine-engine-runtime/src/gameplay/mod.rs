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
    PlayerInteractionTuning, PlayerWeaponState, WeaponAttackKind, WeaponEvent, WeaponEventBus,
    WeaponEventKind, WeaponObstructionState,
};
pub use components::{
    attach_scene_element_core, attach_scene_object_core, scene_entity_by_role,
    AuthoredMapPlacement, AuthoredMapPlacementCloneSource, AuthoredMapPlacementDirty,
    AuthoredMapPlacementReplicaScaleState, AuthoredMapPlacementSource, CharacterBody,
    CharacterMotionTuning, CloudShadowRenderState, CollisionShapeDesc, DisplayMode,
    DisplayVisibility, EnvironmentDomeRenderState, EnvironmentPostFxState, GameRunMode,
    GameplayActor, ModelRenderComponent, PhysicsBodyDesc, PhysicsSurface, PhysicsWorldSettings,
    PlayerActor, PlayerAnimationState, PlayerCharacterPresentation, PlayerCommandFrame,
    PlayerController, PlayerControllerKind, PlayerEvent, PlayerEventBus, PlayerEventKind,
    PlayerFirstPersonCameraAnchor, PlayerFixedPoseHistory, PlayerGroundState,
    PlayerJointRotationWeight, PlayerLocomotionAnimation, PlayerLocomotionState,
    PlayerModelAssignment, PlayerModelBinding, PlayerMovementSpeeds, PlayerRenderPose,
    PlayerSkinBinding, PlayerSkinPose, PlayerSkinVertex, PlayerStanceKind, PlayerStanceState,
    PlayerViewState, PlayerViewVisibility, PlayerViewVisibilityPolicy, PlayerVisualKind,
    PlayerVisualPart, PreparedRenderMesh, PrimitiveGpuEvictionQueue, ResidencyProgress,
    SceneAnchorFollow, SceneEntityAnchor, SceneEntityRole, SkyCloudProfileRenderState,
    StaticMeshCollider, TerrainMaterialLayers, WorldActivationPhase, WorldActivationState,
    WorldAssemblyProgress, WorldClearColor,
};
pub use content::{GameplayContentProvider, GameplayContentProviderRegistry};
pub use execution::{
    GameplayExecutionPhase, GameplayFrame, GameplaySystemProvider, GameplaySystemProviderRegistry,
    GameplayWorld,
};
pub use inventory::{
    active_equipped_weapon_aiming, active_equipped_weapon_binding, active_equipped_weapon_can_aim,
    active_equipped_weapon_can_fire, active_equipped_weapon_can_melee, apply_loadout,
    consume_equipped_ammo, drain_inventory_events, drop_item, ensure_inventory_runtime,
    ensure_player_inventory, equip_first_item, equip_item_instance, equipped_reserve_ammo,
    give_item, inventory_quantity, persist_equipped_weapon_state, play_equipped_weapon_audio,
    play_weapon_item_audio, preload_weapon_audio_definition, remove_item, select_equipment_slot,
    select_highest_ranked_equipped_weapon, spawn_item_pickup, spawn_persistent_item_pickup,
    step_world_items, sync_equipped_weapon_runtime, try_collect_item_pickup, unequip_slot,
    use_item, EquipmentSlot, EquippedWeaponBinding, EquippedWeaponMuzzle, FirearmWeaponDefinition,
    InventoryEntry, InventoryEvent, InventoryEventBus, InventoryEventKind, InventoryLoadout,
    InventoryLoadoutCatalog, InventoryLoadoutEntry, InventoryMutation, ItemCatalog, ItemDefinition,
    ItemId, ItemInstanceId, ItemKind, ItemPickup, ItemUseEffect, MeleeWeaponTuning,
    PlayerInventory, WeaponAnimationDefinition, WeaponAudioAction, WeaponAudioDefinition,
    WeaponCapabilities, WeaponCasingDefinition, WeaponFireMode, WeaponItemDefinition,
    WeaponPresentationDefinition, WeaponType, WorldItemDefinition, WorldItemPresentation,
    WorldItemRuntime, WorldItemVisualPart, SHARED_UNARMED_WEAPON_ITEM_NAME,
};
pub use listeners::{drain_player_events, emit_player_event, sync_player_view_listeners};
pub(crate) use physics::{prewarm_service_physics_backend, sync_prelaunch_service_physics};
pub use physics::{
    PhysicsRuntimeFrameIndex, PhysicsStaticColliderSyncProgress, PhysicsStepTimingTelemetry,
    PhysicsSyncModule,
};
pub use physics_queries::{GameplayPhysicsQueryProvider, GameplayPhysicsQueryProviderRegistry};
pub use player::{
    apply_player_command_frame, apply_player_input, apply_player_stance_geometry,
    attach_active_camera_to_player, capture_player_fixed_poses, clear_player_input,
    clear_player_model_assignment, consume_player_transient_input,
    detach_active_camera_from_player, display_shadow_caster_visible_in_mode,
    display_visible_in_mode, ensure_physics_body, first_player, is_player_controller_enabled,
    player_render_model_matrix, publish_player_render_poses, remove_physics_body,
    set_player_model_assignment, spawn_default_player, spawn_player_controller,
    update_player_animation_states, update_player_stance_camera,
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
