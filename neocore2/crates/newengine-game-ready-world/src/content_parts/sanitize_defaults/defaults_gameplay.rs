use super::*;

pub(in super::super) fn default_status_text() -> String {
    String::new()
}
pub(in super::super) fn default_pickup_status() -> String {
    String::new()
}
pub(in super::super) fn default_target_status() -> String {
    String::new()
}
pub(in super::super) fn default_hazard_status() -> String {
    String::new()
}
pub(in super::super) fn default_goal_locked_status() -> String {
    String::new()
}
pub(in super::super) fn default_goal_complete_status() -> String {
    String::new()
}
pub(in super::super) fn default_failed_progress_label() -> String {
    String::new()
}
pub(in super::super) fn default_completed_progress_label() -> String {
    String::new()
}
pub(in super::super) fn default_player_body_radius() -> f32 {
    0.0
}
pub(in super::super) fn default_player_body_half_height() -> f32 {
    0.0
}
pub(in super::super) fn default_player_visual_radius() -> f32 {
    0.0
}
pub(in super::super) fn default_player_visual_half_height() -> f32 {
    0.0
}
pub(in super::super) fn default_camera_eye_height() -> f32 {
    0.0
}
pub(in super::super) fn default_sprint_multiplier() -> f32 {
    0.0
}
pub(in super::super) fn default_camera_first_person_fov_y_degrees() -> f32 {
    0.0
}
pub(in super::super) fn default_camera_first_person_ads_fov_y_degrees() -> f32 {
    0.0
}
pub(in super::super) fn default_camera_first_person_near() -> f32 {
    0.0
}
pub(in super::super) fn default_camera_first_person_forward_clearance() -> f32 {
    0.0
}
pub(in super::super) fn default_camera_first_person_body_yaw_limit_degrees() -> f32 {
    0.0
}
pub(in super::super) fn default_camera_first_person_down_pitch_limit_degrees() -> f32 {
    0.0
}
pub(in super::super) fn default_camera_third_person_follow_fov_y_degrees() -> f32 {
    0.0
}
pub(in super::super) fn default_camera_third_person_aim_fov_y_degrees() -> f32 {
    0.0
}
pub(in super::super) fn default_camera_third_person_orbit_fov_y_degrees() -> f32 {
    0.0
}
pub(in super::super) fn default_gravity() -> f32 {
    0.0
}
pub(in super::super) fn default_contact_skin() -> f32 {
    0.0
}
pub(in super::super) fn default_terrain_color() -> ColorRgba {
    [0.0; 4]
}
pub(in super::super) fn default_sky_color() -> ColorRgba {
    [0.0; 4]
}
pub(in super::super) fn default_sky_emissive() -> ColorRgb {
    [0.0; 3]
}
pub(in super::super) fn default_tree_bark_color() -> ColorRgba {
    [0.0; 4]
}
pub(in super::super) fn default_tree_leaf_color() -> ColorRgba {
    [0.0; 4]
}
pub(in super::super) fn default_tree_branch_color() -> ColorRgba {
    [0.0; 4]
}
pub(in super::super) fn default_uv_scale() -> [f32; 2] {
    [0.0; 2]
}
pub(in super::super) fn default_uv_offset() -> [f32; 2] {
    [0.0; 2]
}
pub(in super::super) fn default_material_roughness() -> f32 {
    0.0
}
pub(in super::super) fn default_material_normal_scale() -> f32 {
    0.0
}
pub(in super::super) fn default_material_occlusion_strength() -> f32 {
    0.0
}
pub(in super::super) fn default_ambient_color() -> ColorRgb {
    [0.0; 3]
}
pub(in super::super) fn default_ambient_intensity() -> f32 {
    0.0
}
pub(in super::super) fn default_sun_direction() -> ColorRgb {
    [0.0; 3]
}
pub(in super::super) fn default_sun_color() -> ColorRgb {
    [0.0; 3]
}
pub(in super::super) fn default_sun_intensity() -> f32 {
    0.0
}
pub(in super::super) fn default_day_night_enabled() -> bool {
    false
}
pub(in super::super) fn default_time_of_day_hours() -> f32 {
    0.0
}
pub(in super::super) fn default_day_length_seconds() -> f32 {
    0.0
}
pub(in super::super) fn default_day_of_year() -> u32 {
    0
}
pub(in super::super) fn default_sun_latitude_degrees() -> f32 {
    0.0
}
pub(in super::super) fn default_axial_tilt_degrees() -> f32 {
    0.0
}
pub(in super::super) fn default_shadow_enabled() -> bool {
    false
}
pub(in super::super) fn default_shadow_resolution() -> u32 {
    0
}
pub(in super::super) fn default_shadow_cascade_count() -> u32 {
    0
}
pub(in super::super) fn default_shadow_max_distance() -> f32 {
    0.0
}
pub(in super::super) fn default_shadow_softness() -> f32 {
    0.0
}
pub(in super::super) fn default_shadow_bias() -> f32 {
    0.0
}
pub(in super::super) fn default_shadow_normal_bias() -> f32 {
    0.0
}
pub(in super::super) fn default_shadow_contact_strength() -> f32 {
    0.0
}
pub(in super::super) fn default_shadow_filter() -> String {
    String::new()
}
pub(in super::super) fn default_shadow_pcss_light_angular_radius_degrees() -> f32 {
    0.0
}
pub(in super::super) fn default_shadow_pcss_blocker_search_radius_texels() -> f32 {
    0.0
}
pub(in super::super) fn default_shadow_pcss_max_filter_radius_texels() -> f32 {
    0.0
}
pub(in super::super) fn default_shadow_pcss_blocker_samples() -> u32 {
    0
}
pub(in super::super) fn default_shadow_pcss_filter_samples() -> u32 {
    0
}
pub(in super::super) fn default_shadow_pcss_min_filter_radius_texels() -> f32 {
    0.0
}
pub(in super::super) fn default_shadow_pcss_stable_kernel_cell_texels() -> f32 {
    0.0
}
pub(in super::super) fn default_foliage_prefab() -> String {
    String::new()
}
pub(in super::super) fn default_foliage_seed() -> u64 {
    0
}
pub(in super::super) fn default_foliage_grid_min() -> i32 {
    0
}
pub(in super::super) fn default_foliage_grid_max() -> i32 {
    0
}
pub(in super::super) fn default_foliage_spacing() -> f32 {
    0.0
}
pub(in super::super) fn default_foliage_jitter() -> f32 {
    0.0
}
pub(in super::super) fn default_foliage_gate_threshold() -> f32 {
    0.0
}
pub(in super::super) fn default_foliage_min_scale() -> f32 {
    0.0
}
pub(in super::super) fn default_foliage_max_scale() -> f32 {
    0.0
}
pub(in super::super) fn default_foliage_min_player_distance() -> f32 {
    0.0
}
pub(in super::super) fn default_foliage_edge_margin() -> f32 {
    0.0
}
pub(in super::super) fn default_foliage_surface_offset() -> f32 {
    0.0
}
pub(in super::super) fn default_prefab_proxy() -> String {
    String::new()
}
pub(in super::super) fn default_prefab_enabled() -> bool {
    false
}
