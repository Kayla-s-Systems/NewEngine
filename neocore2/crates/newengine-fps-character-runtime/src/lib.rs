#![forbid(unsafe_op_in_unsafe_fn)]

//! Reusable FPS character input, locomotion, grounding and foot-contact mechanics.
//! No project content, mission policy, UI or runtime-profile composition is owned here.

mod character_control;
mod character_physics;
mod noclip;
mod presentation_policy;

pub use character_control::apply_fps_character_commands;
pub use character_physics::{
    collect_character_queries, ensure_footstep_audio_preloaded, resolve_character_query_hits,
    step_character_locomotion, sync_physics_world_settings,
};
pub use noclip::{fps_noclip_enabled, set_fps_noclip, step_fps_noclip_motion, toggle_fps_noclip};

pub use presentation_policy::{
    reconcile_existing_player_assignments_with_policy, PlayableCharacterSelection,
};
