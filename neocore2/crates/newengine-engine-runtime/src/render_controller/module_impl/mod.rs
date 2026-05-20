#![forbid(unsafe_op_in_unsafe_fn)]

mod draw_bucket;
pub(crate) mod draw_lists;
mod external_contribution_lowering;
mod frame_submit;
mod frame_orchestrator;
mod feature_extraction;
mod postfx;
mod frame_envelope_builder;
mod frame_types;
mod gpu_prewarm;
mod input;
pub(super) mod instancing;
mod launch_loading;
mod lifecycle;
pub(crate) mod light_extraction;
pub(crate) mod lights;
pub(crate) mod passes;
mod passes_ubo;
pub(crate) mod pause_menu;
mod profiling;
mod picking;
mod playable_viewport;
mod prelaunch_gate;
mod readiness;
mod render_entry;
pub(crate) mod scene;
mod scene_submit;
mod shadow_cache;
pub(crate) mod shadows;
mod trace_policy;
mod windowing;
mod world_tick;

pub(super) use frame_submit::{record_draw_list, record_render_phase};
pub(super) use super::controller::RuntimeRenderController;
