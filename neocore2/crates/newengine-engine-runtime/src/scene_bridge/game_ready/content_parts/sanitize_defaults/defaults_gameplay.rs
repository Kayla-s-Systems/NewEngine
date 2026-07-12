use super::*;

pub(in super::super) fn default_status_text() -> String {
    String::new()
}
pub(in super::super) fn default_pickup_status() -> String {
    String::new()
}
pub(in super::super) fn default_target_status() -> String {
    "Target neutralized.".to_owned()
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
    0.45
}
pub(in super::super) fn default_player_body_half_height() -> f32 {
    0.45
}
pub(in super::super) fn default_player_visual_radius() -> f32 {
    0.45
}
pub(in super::super) fn default_player_visual_half_height() -> f32 {
    0.90
}
pub(in super::super) fn default_camera_eye_height() -> f32 {
    0.72
}
pub(in super::super) fn default_sprint_multiplier() -> f32 {
    1.75
}
pub(in super::super) fn default_gravity() -> f32 {
    9.81
}
pub(in super::super) fn default_contact_skin() -> f32 {
    0.035
}
pub(in super::super) fn default_terrain_color() -> ColorRgba {
    [0.78, 0.86, 0.68, 1.0]
}
pub(in super::super) fn default_sky_color() -> ColorRgba {
    [0.08, 0.16, 0.34, 1.0]
}
pub(in super::super) fn default_sky_emissive() -> ColorRgb {
    [0.07, 0.14, 0.34]
}
pub(in super::super) fn default_tree_bark_color() -> ColorRgba {
    [0.38, 0.23, 0.12, 1.0]
}
pub(in super::super) fn default_tree_leaf_color() -> ColorRgba {
    [0.18, 0.42, 0.16, 1.0]
}
pub(in super::super) fn default_tree_branch_color() -> ColorRgba {
    [0.32, 0.20, 0.12, 1.0]
}
pub(in super::super) fn default_uv_scale() -> [f32; 2] {
    [1.0, 1.0]
}
pub(in super::super) fn default_uv_offset() -> [f32; 2] {
    [0.0, 0.0]
}
pub(in super::super) fn default_material_roughness() -> f32 {
    0.86
}
pub(in super::super) fn default_material_normal_scale() -> f32 {
    1.0
}
pub(in super::super) fn default_material_occlusion_strength() -> f32 {
    1.0
}
pub(in super::super) fn default_ambient_color() -> ColorRgb {
    [0.42, 0.47, 0.56]
}
pub(in super::super) fn default_ambient_intensity() -> f32 {
    0.36
}
pub(in super::super) fn default_sun_direction() -> ColorRgb {
    [-0.55, -0.82, -0.28]
}
pub(in super::super) fn default_sun_color() -> ColorRgb {
    [1.0, 0.955, 0.86]
}
pub(in super::super) fn default_sun_intensity() -> f32 {
    4.60
}
pub(in super::super) fn default_day_night_enabled() -> bool {
    true
}
pub(in super::super) fn default_time_of_day_hours() -> f32 {
    9.35
}
pub(in super::super) fn default_day_length_seconds() -> f32 {
    720.0
}
pub(in super::super) fn default_day_of_year() -> u32 {
    172
}
pub(in super::super) fn default_sun_latitude_degrees() -> f32 {
    45.0
}
pub(in super::super) fn default_axial_tilt_degrees() -> f32 {
    23.44
}
pub(in super::super) fn default_shadow_enabled() -> bool {
    true
}
pub(in super::super) fn default_shadow_resolution() -> u32 {
    4096
}
pub(in super::super) fn default_shadow_cascade_count() -> u32 {
    4
}
pub(in super::super) fn default_shadow_max_distance() -> f32 {
    180.0
}
pub(in super::super) fn default_shadow_softness() -> f32 {
    0.62
}
pub(in super::super) fn default_shadow_bias() -> f32 {
    0.0025
}
pub(in super::super) fn default_shadow_normal_bias() -> f32 {
    0.015
}
pub(in super::super) fn default_shadow_contact_strength() -> f32 {
    0.58
}
pub(in super::super) fn default_foliage_prefab() -> String {
    String::new()
}
pub(in super::super) fn default_foliage_seed() -> u64 {
    0x5452_4545_2026
}
pub(in super::super) fn default_foliage_grid_min() -> i32 {
    -5
}
pub(in super::super) fn default_foliage_grid_max() -> i32 {
    5
}
pub(in super::super) fn default_foliage_spacing() -> f32 {
    6.0
}
pub(in super::super) fn default_foliage_jitter() -> f32 {
    0.45
}
pub(in super::super) fn default_foliage_gate_threshold() -> f32 {
    0.62
}
pub(in super::super) fn default_foliage_min_scale() -> f32 {
    0.85
}
pub(in super::super) fn default_foliage_max_scale() -> f32 {
    1.35
}
pub(in super::super) fn default_foliage_min_player_distance() -> f32 {
    5.0
}
pub(in super::super) fn default_foliage_edge_margin() -> f32 {
    4.0
}
pub(in super::super) fn default_foliage_surface_offset() -> f32 {
    0.03
}
pub(in super::super) fn default_prefab_proxy() -> String {
    String::new()
}
pub(in super::super) fn default_prefab_enabled() -> bool {
    false
}
