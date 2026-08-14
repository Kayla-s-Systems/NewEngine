use super::*;
use newengine_game_data::default_game_data;

pub(in super::super) fn default_sky_definition_ref() -> String {
    default_game_data().world.sky.definition_ref.clone()
}
pub(in super::super) fn default_sky_radius() -> f32 {
    default_game_data().world.sky.radius
}
pub(in super::super) fn default_skydome_mesh() -> String {
    default_game_data().world.sky.mesh.clone()
}
pub(in super::super) fn default_sky_follow_camera() -> bool {
    default_game_data().world.sky.follow_camera
}
pub(in super::super) fn default_cloud_dictionary() -> String {
    default_game_data().world.sky.cloud_dictionary.clone()
}
pub(in super::super) fn default_cloud_profile() -> String {
    default_game_data().world.sky.cloud_profile.clone()
}
pub(in super::super) fn default_sky_sun_radius() -> f32 {
    default_game_data().world.sky.sun_radius
}
pub(in super::super) fn default_sky_moon_radius() -> f32 {
    default_game_data().world.sky.moon_radius
}
pub(in super::super) fn default_moon_texture() -> String {
    default_game_data().world.sky.moon_texture.clone()
}
pub(in super::super) fn default_sky_day_zenith() -> ColorRgb {
    default_game_data().world.sky.atmosphere.day_zenith
}
pub(in super::super) fn default_sky_day_horizon() -> ColorRgb {
    default_game_data().world.sky.atmosphere.day_horizon
}
pub(in super::super) fn default_sky_dusk_zenith() -> ColorRgb {
    default_game_data().world.sky.atmosphere.dusk_zenith
}
pub(in super::super) fn default_sky_dusk_horizon() -> ColorRgb {
    default_game_data().world.sky.atmosphere.dusk_horizon
}
pub(in super::super) fn default_sky_night_zenith() -> ColorRgb {
    default_game_data().world.sky.atmosphere.night_zenith
}
pub(in super::super) fn default_sky_night_horizon() -> ColorRgb {
    default_game_data().world.sky.atmosphere.night_horizon
}
pub(in super::super) fn default_sky_cloud_day() -> ColorRgb {
    default_game_data().world.sky.atmosphere.cloud_day
}
pub(in super::super) fn default_sky_cloud_night() -> ColorRgb {
    default_game_data().world.sky.atmosphere.cloud_night
}
pub(in super::super) fn default_sky_night_strength() -> f32 {
    default_game_data().world.sky.atmosphere.night_sky_strength
}
pub(in super::super) fn default_sky_cloud_coverage() -> f32 {
    default_game_data().world.sky.atmosphere.cloud_coverage
}
pub(in super::super) fn default_sky_cloud_softness() -> f32 {
    default_game_data().world.sky.atmosphere.cloud_softness
}
