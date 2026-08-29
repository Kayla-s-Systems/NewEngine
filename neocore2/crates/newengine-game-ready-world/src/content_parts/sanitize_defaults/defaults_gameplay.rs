use super::*;
use newengine_game_data::default_game_data;

pub(in super::super) fn default_status_text() -> String {
    default_game_data().gameplay.status.default_status.clone()
}
pub(in super::super) fn default_pickup_status() -> String {
    default_game_data().gameplay.status.pickup_status.clone()
}
pub(in super::super) fn default_target_status() -> String {
    default_game_data().gameplay.status.target_status.clone()
}
pub(in super::super) fn default_hazard_status() -> String {
    default_game_data().gameplay.status.hazard_status.clone()
}
pub(in super::super) fn default_goal_locked_status() -> String {
    default_game_data()
        .gameplay
        .status
        .goal_locked_status
        .clone()
}
pub(in super::super) fn default_goal_complete_status() -> String {
    default_game_data()
        .gameplay
        .status
        .goal_complete_status
        .clone()
}
pub(in super::super) fn default_failed_progress_label() -> String {
    default_game_data()
        .gameplay
        .status
        .failed_progress_label
        .clone()
}
pub(in super::super) fn default_completed_progress_label() -> String {
    default_game_data()
        .gameplay
        .status
        .completed_progress_label
        .clone()
}
pub(in super::super) fn default_player_body_radius() -> f32 {
    default_game_data().player.tuning.body_radius
}
pub(in super::super) fn default_player_body_half_height() -> f32 {
    default_game_data().player.tuning.body_half_height
}
pub(in super::super) fn default_player_visual_radius() -> f32 {
    default_game_data().player.tuning.visual_radius
}
pub(in super::super) fn default_player_visual_half_height() -> f32 {
    default_game_data().player.tuning.visual_half_height
}
pub(in super::super) fn default_camera_eye_height() -> f32 {
    default_game_data().player.tuning.camera_eye_height
}
pub(in super::super) fn default_sprint_multiplier() -> f32 {
    default_game_data().player.tuning.sprint_multiplier
}
pub(in super::super) fn default_gravity() -> f32 {
    default_game_data().player.tuning.gravity
}
pub(in super::super) fn default_contact_skin() -> f32 {
    default_game_data().player.tuning.contact_skin
}
pub(in super::super) fn default_terrain_color() -> ColorRgba {
    default_game_data().world.palette.terrain
}
pub(in super::super) fn default_sky_color() -> ColorRgba {
    default_game_data().world.palette.sky
}
pub(in super::super) fn default_sky_emissive() -> ColorRgb {
    default_game_data().world.palette.sky_emissive
}
pub(in super::super) fn default_tree_bark_color() -> ColorRgba {
    default_game_data().world.palette.tree_bark
}
pub(in super::super) fn default_tree_leaf_color() -> ColorRgba {
    default_game_data().world.palette.tree_leaf
}
pub(in super::super) fn default_tree_branch_color() -> ColorRgba {
    default_game_data().world.palette.tree_branch
}
pub(in super::super) fn default_uv_scale() -> [f32; 2] {
    default_game_data().world.material.uv_scale
}
pub(in super::super) fn default_uv_offset() -> [f32; 2] {
    default_game_data().world.material.uv_offset
}
pub(in super::super) fn default_material_roughness() -> f32 {
    default_game_data().world.material.roughness
}
pub(in super::super) fn default_material_normal_scale() -> f32 {
    default_game_data().world.material.normal_scale
}
pub(in super::super) fn default_material_occlusion_strength() -> f32 {
    default_game_data().world.material.occlusion_strength
}
pub(in super::super) fn default_ambient_color() -> ColorRgb {
    default_game_data().world.lighting.ambient_color
}
pub(in super::super) fn default_ambient_intensity() -> f32 {
    default_game_data().world.lighting.ambient_intensity
}
pub(in super::super) fn default_sun_direction() -> ColorRgb {
    default_game_data().world.lighting.sun_direction
}
pub(in super::super) fn default_sun_color() -> ColorRgb {
    default_game_data().world.lighting.sun_color
}
pub(in super::super) fn default_sun_intensity() -> f32 {
    default_game_data().world.lighting.sun_intensity
}
pub(in super::super) fn default_day_night_enabled() -> bool {
    default_game_data().world.day_night.enabled
}
pub(in super::super) fn default_time_of_day_hours() -> f32 {
    default_game_data().world.day_night.time_of_day_hours
}
pub(in super::super) fn default_day_length_seconds() -> f32 {
    default_game_data().world.day_night.day_length_seconds
}
pub(in super::super) fn default_day_of_year() -> u32 {
    default_game_data().world.day_night.day_of_year
}
pub(in super::super) fn default_sun_latitude_degrees() -> f32 {
    default_game_data().world.day_night.latitude_degrees
}
pub(in super::super) fn default_axial_tilt_degrees() -> f32 {
    default_game_data().world.day_night.axial_tilt_degrees
}
pub(in super::super) fn default_shadow_enabled() -> bool {
    default_game_data().world.shadows.enabled
}
pub(in super::super) fn default_shadow_resolution() -> u32 {
    default_game_data().world.shadows.resolution
}
pub(in super::super) fn default_shadow_cascade_count() -> u32 {
    default_game_data().world.shadows.cascade_count
}
pub(in super::super) fn default_shadow_max_distance() -> f32 {
    default_game_data().world.shadows.max_distance
}
pub(in super::super) fn default_shadow_softness() -> f32 {
    default_game_data().world.shadows.softness
}
pub(in super::super) fn default_shadow_bias() -> f32 {
    default_game_data().world.shadows.bias
}
pub(in super::super) fn default_shadow_normal_bias() -> f32 {
    default_game_data().world.shadows.normal_bias
}
pub(in super::super) fn default_shadow_contact_strength() -> f32 {
    default_game_data().world.shadows.contact_strength
}
pub(in super::super) fn default_shadow_filter() -> String {
    default_game_data().world.shadows.filter.clone()
}
pub(in super::super) fn default_shadow_pcss_light_angular_radius_degrees() -> f32 {
    default_game_data()
        .world
        .shadows
        .pcss_light_angular_radius_degrees
}
pub(in super::super) fn default_shadow_pcss_blocker_search_radius_texels() -> f32 {
    default_game_data()
        .world
        .shadows
        .pcss_blocker_search_radius_texels
}
pub(in super::super) fn default_shadow_pcss_max_filter_radius_texels() -> f32 {
    default_game_data()
        .world
        .shadows
        .pcss_max_filter_radius_texels
}
pub(in super::super) fn default_shadow_pcss_blocker_samples() -> u32 {
    default_game_data().world.shadows.pcss_blocker_samples
}
pub(in super::super) fn default_shadow_pcss_filter_samples() -> u32 {
    default_game_data().world.shadows.pcss_filter_samples
}
pub(in super::super) fn default_shadow_pcss_min_filter_radius_texels() -> f32 {
    default_game_data()
        .world
        .shadows
        .pcss_min_filter_radius_texels
}
pub(in super::super) fn default_shadow_pcss_stable_kernel_cell_texels() -> f32 {
    default_game_data()
        .world
        .shadows
        .pcss_stable_kernel_cell_texels
}
pub(in super::super) fn default_foliage_prefab() -> String {
    default_game_data().world.foliage.prefab.clone()
}
pub(in super::super) fn default_foliage_seed() -> u64 {
    default_game_data().world.foliage.seed
}
pub(in super::super) fn default_foliage_grid_min() -> i32 {
    default_game_data().world.foliage.grid_min
}
pub(in super::super) fn default_foliage_grid_max() -> i32 {
    default_game_data().world.foliage.grid_max
}
pub(in super::super) fn default_foliage_spacing() -> f32 {
    default_game_data().world.foliage.spacing
}
pub(in super::super) fn default_foliage_jitter() -> f32 {
    default_game_data().world.foliage.jitter
}
pub(in super::super) fn default_foliage_gate_threshold() -> f32 {
    default_game_data().world.foliage.gate_threshold
}
pub(in super::super) fn default_foliage_min_scale() -> f32 {
    default_game_data().world.foliage.min_scale
}
pub(in super::super) fn default_foliage_max_scale() -> f32 {
    default_game_data().world.foliage.max_scale
}
pub(in super::super) fn default_foliage_min_player_distance() -> f32 {
    default_game_data().world.foliage.min_player_distance
}
pub(in super::super) fn default_foliage_edge_margin() -> f32 {
    default_game_data().world.foliage.edge_margin
}
pub(in super::super) fn default_foliage_surface_offset() -> f32 {
    default_game_data().world.foliage.surface_offset
}
pub(in super::super) fn default_prefab_proxy() -> String {
    String::new()
}
pub(in super::super) fn default_prefab_enabled() -> bool {
    false
}
