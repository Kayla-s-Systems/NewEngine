use super::*;

#[path = "sky_sampling/environment.rs"]
mod environment;
#[path = "sky_sampling/math.rs"]
mod math;
#[path = "sky_sampling/preset.rs"]
mod preset;
#[path = "sky_sampling/sample.rs"]
mod sample;
#[path = "sky_sampling/time.rs"]
mod time;

pub(crate) use environment::sample_sky_frame_from_environment;
pub(crate) use math::{
    sky_clamp3, sky_color_to_rgba, sky_lerp3, sky_mul3, sky_mul3_components, sky_safe_dir,
    sky_smoothstep, solar_direction_from_cycle,
};
pub(crate) use sample::{env_color_to_rgb, env_vec_to_vec3, sample_sky_frame};
pub(crate) use time::{
    authored_time_snapshot_for_sky_cycle, environment_frame_for_sky_cycle,
    sync_game_ready_day_night_to_engine_time, time_snapshot_for_sky_cycle,
};

use preset::sky_cloud_visual_preset;
