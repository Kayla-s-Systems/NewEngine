use super::*;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PlayerCameraViewMode {
    #[default]
    FirstPerson,
    ThirdPersonFollow,
    ThirdPersonAim,
    ThirdPersonOrbit,
}

/// Project-authored gameplay camera contract consumed by the generic camera gateway.
///
/// The engine owns camera mechanics, but not game-specific lens, first-person visibility,
/// or local-owner framing policy. Runtime profiles install this component from project/scene data.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlayerCameraProfile {
    pub initial_view: PlayerCameraViewMode,
    pub first_person_fov_y_radians: f32,
    pub first_person_ads_fov_y_radians: f32,
    pub first_person_near: f32,
    pub first_person_forward_clearance: f32,
    pub first_person_body_yaw_limit_radians: f32,
    pub first_person_down_pitch_limit_radians: f32,
    pub first_person_collision_enabled: bool,
    pub first_person_collision_probe_radius: f32,
    pub first_person_collision_padding: f32,
    /// Ground-contact micro-jitter deadband applied only to the render-cadence FPP eye Y anchor.
    pub first_person_grounded_eye_deadband_m: f32,
    /// Time constant for meaningful grounded FPP eye-height changes outside the deadband.
    pub first_person_grounded_eye_time_constant_seconds: f32,
    /// Camera-space share of weapon recoil. Weapon animation remains the primary recoil owner.
    pub first_person_camera_recoil_share: f32,
    /// Response frequency for the semantic hip -> ADS camera blend.
    pub first_person_aim_response_hz: f32,
    /// Dynamic near-plane scanner. The engine owns the query mechanism; these limits are authored.
    pub near_clip_enabled: bool,
    pub near_clip_first_person_max_distance: f32,
    pub near_clip_third_person_min_distance: f32,
    pub near_clip_third_person_max_distance: f32,
    pub near_clip_pull_in_distance: f32,
    pub near_clip_probe_radius: f32,
    pub near_clip_release_time_seconds: f32,
    pub near_clip_hysteresis_m: f32,
    pub third_person_follow_fov_y_radians: f32,
    pub third_person_follow_offset_ls: Vec3,
    pub third_person_follow_focus_offset_ls: Vec3,
    pub third_person_follow_smooth_time: f32,
    pub third_person_follow_max_speed: f32,
    pub third_person_follow_zoom_min: f32,
    pub third_person_follow_zoom_max: f32,
    pub third_person_aim_fov_y_radians: f32,
    pub third_person_aim_offset_ls: Vec3,
    pub third_person_aim_focus_offset_ls: Vec3,
    pub third_person_aim_smooth_time: f32,
    pub third_person_aim_max_speed: f32,
    pub third_person_aim_zoom_min: f32,
    pub third_person_aim_zoom_max: f32,
    pub third_person_orbit_fov_y_radians: f32,
    pub third_person_orbit_offset_ls: Vec3,
    pub third_person_orbit_focus_offset_ls: Vec3,
    pub third_person_orbit_smooth_time: f32,
    pub third_person_orbit_max_speed: f32,
    pub third_person_orbit_zoom_min: f32,
    pub third_person_orbit_zoom_max: f32,
    pub third_person_orbit_look_sensitivity_radians_per_pixel: f32,
    pub third_person_orbit_pitch_min_radians: f32,
    pub third_person_orbit_pitch_max_radians: f32,
    pub third_person_collision_enabled: bool,
    pub third_person_collision_probe_radius: f32,
    pub third_person_collision_padding: f32,
    pub third_person_collision_min_distance: f32,
    /// Natural frequency of collision pull-back after occluding geometry clears.
    pub third_person_collision_release_frequency_hz: f32,
    pub third_person_collision_release_damping_ratio: f32,
    pub third_person_collision_distance_hysteresis: f32,
    /// Blend between the ideal pre-collision look-at and collision-adjusted look-at response.
    pub third_person_look_at_collision_blend: f32,
    pub third_person_look_at_response_hz: f32,
    pub third_person_look_at_max_error_fov_fraction: f32,
    /// Orbit-relative catch-up used on gameplay camera entry/mode switches; player translation stays exact.
    pub third_person_catch_up_enabled: bool,
    pub third_person_catch_up_frequency_hz: f32,
    pub third_person_catch_up_damping_ratio: f32,
    pub third_person_catch_up_max_distance_m: f32,
    pub third_person_catch_up_settle_distance_m: f32,
    pub zoom_wheel_exponent_per_step: f32,
    pub orbit_drag_zoom_exponent_per_pixel: f32,
    pub zoom_smooth_time_seconds: f32,
    pub gameplay_blend_in_seconds: f32,
    pub gameplay_blend_out_seconds: f32,
    pub gameplay_blend_lock_input: bool,
    /// Legacy compatibility input. Full-body first-person runtimes keep the local world model visible
    /// and ignore whole-avatar hiding; only camera-near shells may use view visibility masking.
    pub hide_local_model_in_first_person: bool,
}

impl PlayerCameraProfile {
    #[inline]
    pub fn sanitized(self) -> Self {
        let defaults = Self::default();
        let fov = |value: f32, fallback: f32| {
            finite_or(value, fallback).clamp(20.0_f32.to_radians(), 130.0_f32.to_radians())
        };
        let positive =
            |value: f32, fallback: f32, max: f32| finite_or(value, fallback).clamp(0.0, max);
        let vec3 = |value: Vec3, fallback: Vec3| if value.is_finite() { value } else { fallback };
        let zoom_pair = |min_value: f32, max_value: f32, fallback_min: f32, fallback_max: f32| {
            let min_value = finite_or(min_value, fallback_min).clamp(0.01, 1000.0);
            let max_value = finite_or(max_value, fallback_max).clamp(min_value, 1000.0);
            (min_value, max_value)
        };
        let (follow_zoom_min, follow_zoom_max) = zoom_pair(
            self.third_person_follow_zoom_min,
            self.third_person_follow_zoom_max,
            defaults.third_person_follow_zoom_min,
            defaults.third_person_follow_zoom_max,
        );
        let (aim_zoom_min, aim_zoom_max) = zoom_pair(
            self.third_person_aim_zoom_min,
            self.third_person_aim_zoom_max,
            defaults.third_person_aim_zoom_min,
            defaults.third_person_aim_zoom_max,
        );
        let (orbit_zoom_min, orbit_zoom_max) = zoom_pair(
            self.third_person_orbit_zoom_min,
            self.third_person_orbit_zoom_max,
            defaults.third_person_orbit_zoom_min,
            defaults.third_person_orbit_zoom_max,
        );
        Self {
            initial_view: self.initial_view,
            first_person_fov_y_radians: fov(
                self.first_person_fov_y_radians,
                defaults.first_person_fov_y_radians,
            ),
            first_person_ads_fov_y_radians: fov(
                self.first_person_ads_fov_y_radians,
                defaults.first_person_ads_fov_y_radians,
            ),
            first_person_near: finite_or(self.first_person_near, defaults.first_person_near)
                .clamp(0.005, 0.50),
            first_person_forward_clearance: finite_or(
                self.first_person_forward_clearance,
                defaults.first_person_forward_clearance,
            )
            .clamp(0.0, 0.25),
            first_person_body_yaw_limit_radians: finite_or(
                self.first_person_body_yaw_limit_radians,
                defaults.first_person_body_yaw_limit_radians,
            )
            .clamp(1.0_f32.to_radians(), 179.0_f32.to_radians()),
            first_person_down_pitch_limit_radians: finite_or(
                self.first_person_down_pitch_limit_radians,
                defaults.first_person_down_pitch_limit_radians,
            )
            .clamp(1.0_f32.to_radians(), 89.0_f32.to_radians()),
            first_person_collision_enabled: self.first_person_collision_enabled,
            first_person_collision_probe_radius: positive(
                self.first_person_collision_probe_radius,
                defaults.first_person_collision_probe_radius,
                1.0,
            ),
            first_person_collision_padding: positive(
                self.first_person_collision_padding,
                defaults.first_person_collision_padding,
                0.5,
            ),
            first_person_grounded_eye_deadband_m: positive(
                self.first_person_grounded_eye_deadband_m,
                defaults.first_person_grounded_eye_deadband_m,
                0.25,
            ),
            first_person_grounded_eye_time_constant_seconds: positive(
                self.first_person_grounded_eye_time_constant_seconds,
                defaults.first_person_grounded_eye_time_constant_seconds,
                5.0,
            )
            .max(0.001),
            first_person_camera_recoil_share: positive(
                self.first_person_camera_recoil_share,
                defaults.first_person_camera_recoil_share,
                2.0,
            ),
            first_person_aim_response_hz: positive(
                self.first_person_aim_response_hz,
                defaults.first_person_aim_response_hz,
                120.0,
            )
            .max(0.01),
            near_clip_enabled: self.near_clip_enabled,
            near_clip_first_person_max_distance: finite_or(
                self.near_clip_first_person_max_distance,
                defaults.near_clip_first_person_max_distance,
            )
            .clamp(self.first_person_near.max(0.005), 2.0),
            near_clip_third_person_min_distance: positive(
                self.near_clip_third_person_min_distance,
                defaults.near_clip_third_person_min_distance,
                2.0,
            )
            .max(0.005),
            near_clip_third_person_max_distance: finite_or(
                self.near_clip_third_person_max_distance,
                defaults.near_clip_third_person_max_distance,
            )
            .clamp(
                positive(
                    self.near_clip_third_person_min_distance,
                    defaults.near_clip_third_person_min_distance,
                    2.0,
                )
                .max(0.005),
                4.0,
            ),
            near_clip_pull_in_distance: positive(
                self.near_clip_pull_in_distance,
                defaults.near_clip_pull_in_distance,
                0.5,
            ),
            near_clip_probe_radius: positive(
                self.near_clip_probe_radius,
                defaults.near_clip_probe_radius,
                0.25,
            ),
            near_clip_release_time_seconds: positive(
                self.near_clip_release_time_seconds,
                defaults.near_clip_release_time_seconds,
                5.0,
            )
            .max(0.001),
            near_clip_hysteresis_m: positive(
                self.near_clip_hysteresis_m,
                defaults.near_clip_hysteresis_m,
                0.25,
            ),
            third_person_follow_fov_y_radians: fov(
                self.third_person_follow_fov_y_radians,
                defaults.third_person_follow_fov_y_radians,
            ),
            third_person_follow_offset_ls: vec3(
                self.third_person_follow_offset_ls,
                defaults.third_person_follow_offset_ls,
            ),
            third_person_follow_focus_offset_ls: vec3(
                self.third_person_follow_focus_offset_ls,
                defaults.third_person_follow_focus_offset_ls,
            ),
            third_person_follow_smooth_time: positive(
                self.third_person_follow_smooth_time,
                defaults.third_person_follow_smooth_time,
                10.0,
            ),
            third_person_follow_max_speed: positive(
                self.third_person_follow_max_speed,
                defaults.third_person_follow_max_speed,
                1000.0,
            ),
            third_person_follow_zoom_min: follow_zoom_min,
            third_person_follow_zoom_max: follow_zoom_max,
            third_person_aim_fov_y_radians: fov(
                self.third_person_aim_fov_y_radians,
                defaults.third_person_aim_fov_y_radians,
            ),
            third_person_aim_offset_ls: vec3(
                self.third_person_aim_offset_ls,
                defaults.third_person_aim_offset_ls,
            ),
            third_person_aim_focus_offset_ls: vec3(
                self.third_person_aim_focus_offset_ls,
                defaults.third_person_aim_focus_offset_ls,
            ),
            third_person_aim_smooth_time: positive(
                self.third_person_aim_smooth_time,
                defaults.third_person_aim_smooth_time,
                10.0,
            ),
            third_person_aim_max_speed: positive(
                self.third_person_aim_max_speed,
                defaults.third_person_aim_max_speed,
                1000.0,
            ),
            third_person_aim_zoom_min: aim_zoom_min,
            third_person_aim_zoom_max: aim_zoom_max,
            third_person_orbit_fov_y_radians: fov(
                self.third_person_orbit_fov_y_radians,
                defaults.third_person_orbit_fov_y_radians,
            ),
            third_person_orbit_offset_ls: vec3(
                self.third_person_orbit_offset_ls,
                defaults.third_person_orbit_offset_ls,
            ),
            third_person_orbit_focus_offset_ls: vec3(
                self.third_person_orbit_focus_offset_ls,
                defaults.third_person_orbit_focus_offset_ls,
            ),
            third_person_orbit_smooth_time: positive(
                self.third_person_orbit_smooth_time,
                defaults.third_person_orbit_smooth_time,
                10.0,
            ),
            third_person_orbit_max_speed: positive(
                self.third_person_orbit_max_speed,
                defaults.third_person_orbit_max_speed,
                1000.0,
            ),
            third_person_orbit_zoom_min: orbit_zoom_min,
            third_person_orbit_zoom_max: orbit_zoom_max,
            third_person_orbit_look_sensitivity_radians_per_pixel: positive(
                self.third_person_orbit_look_sensitivity_radians_per_pixel,
                defaults.third_person_orbit_look_sensitivity_radians_per_pixel,
                0.25,
            ),
            third_person_orbit_pitch_min_radians: finite_or(
                self.third_person_orbit_pitch_min_radians,
                defaults.third_person_orbit_pitch_min_radians,
            )
            .clamp(-89.0_f32.to_radians(), 89.0_f32.to_radians()),
            third_person_orbit_pitch_max_radians: finite_or(
                self.third_person_orbit_pitch_max_radians,
                defaults.third_person_orbit_pitch_max_radians,
            )
            .clamp(-89.0_f32.to_radians(), 89.0_f32.to_radians())
            .max(
                finite_or(
                    self.third_person_orbit_pitch_min_radians,
                    defaults.third_person_orbit_pitch_min_radians,
                )
                .clamp(-89.0_f32.to_radians(), 89.0_f32.to_radians())
                    + 1.0_f32.to_radians(),
            ),
            third_person_collision_enabled: self.third_person_collision_enabled,
            third_person_collision_probe_radius: positive(
                self.third_person_collision_probe_radius,
                defaults.third_person_collision_probe_radius,
                4.0,
            ),
            third_person_collision_padding: positive(
                self.third_person_collision_padding,
                defaults.third_person_collision_padding,
                2.0,
            ),
            third_person_collision_min_distance: positive(
                self.third_person_collision_min_distance,
                defaults.third_person_collision_min_distance,
                32.0,
            ),
            third_person_collision_release_frequency_hz: positive(
                self.third_person_collision_release_frequency_hz,
                defaults.third_person_collision_release_frequency_hz,
                60.0,
            )
            .max(0.01),
            third_person_collision_release_damping_ratio: finite_or(
                self.third_person_collision_release_damping_ratio,
                defaults.third_person_collision_release_damping_ratio,
            )
            .clamp(0.05, 4.0),
            third_person_collision_distance_hysteresis: positive(
                self.third_person_collision_distance_hysteresis,
                defaults.third_person_collision_distance_hysteresis,
                0.25,
            ),
            third_person_look_at_collision_blend: finite_or(
                self.third_person_look_at_collision_blend,
                defaults.third_person_look_at_collision_blend,
            )
            .clamp(0.0, 1.0),
            third_person_look_at_response_hz: positive(
                self.third_person_look_at_response_hz,
                defaults.third_person_look_at_response_hz,
                120.0,
            )
            .max(0.01),
            third_person_look_at_max_error_fov_fraction: finite_or(
                self.third_person_look_at_max_error_fov_fraction,
                defaults.third_person_look_at_max_error_fov_fraction,
            )
            .clamp(0.0, 1.0),
            third_person_catch_up_enabled: self.third_person_catch_up_enabled,
            third_person_catch_up_frequency_hz: positive(
                self.third_person_catch_up_frequency_hz,
                defaults.third_person_catch_up_frequency_hz,
                60.0,
            )
            .max(0.01),
            third_person_catch_up_damping_ratio: finite_or(
                self.third_person_catch_up_damping_ratio,
                defaults.third_person_catch_up_damping_ratio,
            )
            .clamp(0.05, 4.0),
            third_person_catch_up_max_distance_m: positive(
                self.third_person_catch_up_max_distance_m,
                defaults.third_person_catch_up_max_distance_m,
                100.0,
            )
            .max(0.01),
            third_person_catch_up_settle_distance_m: positive(
                self.third_person_catch_up_settle_distance_m,
                defaults.third_person_catch_up_settle_distance_m,
                1.0,
            ),
            zoom_wheel_exponent_per_step: positive(
                self.zoom_wheel_exponent_per_step,
                defaults.zoom_wheel_exponent_per_step,
                4.0,
            ),
            orbit_drag_zoom_exponent_per_pixel: positive(
                self.orbit_drag_zoom_exponent_per_pixel,
                defaults.orbit_drag_zoom_exponent_per_pixel,
                1.0,
            ),
            zoom_smooth_time_seconds: positive(
                self.zoom_smooth_time_seconds,
                defaults.zoom_smooth_time_seconds,
                5.0,
            )
            .max(0.001),
            gameplay_blend_in_seconds: positive(
                self.gameplay_blend_in_seconds,
                defaults.gameplay_blend_in_seconds,
                30.0,
            ),
            gameplay_blend_out_seconds: positive(
                self.gameplay_blend_out_seconds,
                defaults.gameplay_blend_out_seconds,
                30.0,
            ),
            gameplay_blend_lock_input: self.gameplay_blend_lock_input,
            hide_local_model_in_first_person: self.hide_local_model_in_first_person,
        }
    }
}

impl Default for PlayerCameraProfile {
    #[inline]
    fn default() -> Self {
        Self {
            initial_view: PlayerCameraViewMode::FirstPerson,
            first_person_fov_y_radians: 68.0_f32.to_radians(),
            first_person_ads_fov_y_radians: 45.0_f32.to_radians(),
            first_person_near: 0.045,
            first_person_forward_clearance: 0.07,
            first_person_body_yaw_limit_radians: 65.0_f32.to_radians(),
            first_person_down_pitch_limit_radians: 85.0_f32.to_radians(),
            first_person_collision_enabled: true,
            first_person_collision_probe_radius: 0.055,
            first_person_collision_padding: 0.012,
            first_person_grounded_eye_deadband_m: 0.010,
            first_person_grounded_eye_time_constant_seconds: 0.060,
            first_person_camera_recoil_share: 0.42,
            first_person_aim_response_hz: 18.0,
            near_clip_enabled: true,
            near_clip_first_person_max_distance: 0.09,
            near_clip_third_person_min_distance: 0.05,
            near_clip_third_person_max_distance: 0.28,
            near_clip_pull_in_distance: 0.018,
            near_clip_probe_radius: 0.010,
            near_clip_release_time_seconds: 0.08,
            near_clip_hysteresis_m: 0.0025,
            third_person_follow_fov_y_radians: 64.0_f32.to_radians(),
            third_person_follow_offset_ls: Vec3::new(0.35, 1.65, 4.5),
            third_person_follow_focus_offset_ls: Vec3::new(0.0, 0.95, 0.0),
            third_person_follow_smooth_time: 0.08,
            third_person_follow_max_speed: 0.0,
            third_person_follow_zoom_min: 1.35,
            third_person_follow_zoom_max: 9.0,
            third_person_aim_fov_y_radians: 54.0_f32.to_radians(),
            third_person_aim_offset_ls: Vec3::new(0.55, 1.55, 2.2),
            third_person_aim_focus_offset_ls: Vec3::new(0.0, 1.25, 0.0),
            third_person_aim_smooth_time: 0.035,
            third_person_aim_max_speed: 0.0,
            third_person_aim_zoom_min: 1.10,
            third_person_aim_zoom_max: 4.50,
            third_person_orbit_fov_y_radians: 60.0_f32.to_radians(),
            third_person_orbit_offset_ls: Vec3::new(0.0, 0.0, 4.8),
            third_person_orbit_focus_offset_ls: Vec3::new(0.0, 0.95, 0.0),
            third_person_orbit_smooth_time: 0.06,
            third_person_orbit_max_speed: 0.0,
            third_person_orbit_zoom_min: 1.35,
            third_person_orbit_zoom_max: 10.0,
            third_person_orbit_look_sensitivity_radians_per_pixel: 0.0028,
            third_person_orbit_pitch_min_radians: -70.0_f32.to_radians(),
            third_person_orbit_pitch_max_radians: 45.0_f32.to_radians(),
            third_person_collision_enabled: true,
            third_person_collision_probe_radius: 0.18,
            third_person_collision_padding: 0.08,
            third_person_collision_min_distance: 0.75,
            third_person_collision_release_frequency_hz: 1.6,
            third_person_collision_release_damping_ratio: 0.8,
            third_person_collision_distance_hysteresis: 0.005,
            third_person_look_at_collision_blend: 0.70,
            third_person_look_at_response_hz: 14.0,
            third_person_look_at_max_error_fov_fraction: 0.12,
            third_person_catch_up_enabled: true,
            third_person_catch_up_frequency_hz: 2.4,
            third_person_catch_up_damping_ratio: 1.0,
            third_person_catch_up_max_distance_m: 8.0,
            third_person_catch_up_settle_distance_m: 0.006,
            zoom_wheel_exponent_per_step: 0.16,
            orbit_drag_zoom_exponent_per_pixel: 0.008,
            zoom_smooth_time_seconds: 0.09,
            gameplay_blend_in_seconds: 0.16,
            gameplay_blend_out_seconds: 0.14,
            gameplay_blend_lock_input: false,
            hide_local_model_in_first_person: false,
        }
    }
}
