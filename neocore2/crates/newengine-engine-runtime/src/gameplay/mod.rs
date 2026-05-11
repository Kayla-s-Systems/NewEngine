#![forbid(unsafe_op_in_unsafe_fn)]

mod components;
mod fps_demo;
mod physics;
mod player;
mod schedule;
mod snapshot;

pub use components::{
    CollisionBody, CollisionShape, DisplayMode, DisplayVisibility, EditorPlayMode, FpsDemoGoal,
    FpsDemoHazard, FpsDemoPickup, FpsDemoRules, FpsDemoState, FpsPlayerTuning, GameplayActor,
    PlayerActor,
};
pub use fps_demo::step_fps_demo_gameplay;
pub use player::{
    apply_player_input, attach_active_camera_to_player, clear_player_input,
    detach_active_camera_from_player, display_visible_in_mode, ensure_collision_body, first_player,
    remove_collision_body, spawn_default_player, spawn_default_player_with_tuning,
};
pub use schedule::{default_sim_schedule, run_schedule};
pub use snapshot::{
    capture_runtime_world_snapshot, restore_runtime_world_snapshot, RuntimeEntitySnapshot,
    RuntimeWorldSnapshot,
};
