#![forbid(unsafe_op_in_unsafe_fn)]

mod components;
mod fps_demo;
mod listeners;
mod physics;
mod player;
mod schedule;
mod snapshot;

pub use components::{
    CollisionShapeDesc, DisplayMode, DisplayVisibility, FpsDemoGoal, FpsDemoHazard, FpsDemoPickup,
    FpsDemoRules, FpsDemoState, FpsPlayerTuning, GameReadyWorldLaunchGate, GameRunMode,
    GameplayActor, PhysicsBodyDesc, PlayerActor, PlayerController, PlayerControllerKind,
    PlayerEvent, PlayerEventBus, PlayerEventKind, PlayerModelBinding, PlayerViewVisibility,
    PlayerViewVisibilityPolicy, PlayerVisualKind, PlayerVisualPart,
};
pub use fps_demo::step_fps_demo_gameplay;
pub use listeners::{drain_player_events, emit_player_event, sync_player_view_listeners};
pub use physics::{PhysicsRuntimeFrameIndex, PhysicsSyncModule};
pub use player::{
    apply_player_input, attach_active_camera_to_player, clear_player_input,
    detach_active_camera_from_player, display_visible_in_mode, ensure_physics_body, first_player,
    is_player_controller_enabled, remove_physics_body, spawn_default_player,
    spawn_default_player_with_tuning, spawn_player_controller_with_tuning,
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
