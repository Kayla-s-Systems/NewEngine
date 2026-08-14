#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_game_data::default_game_data;

/// Declarative FPS runtime tuning.
///
/// The scene/profile owns these values; runtime systems only consume the resource.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FpsPlayerTuning {
    pub body_radius: f32,
    pub body_half_height: f32,
    pub crouched_body_half_height: f32,
    pub visual_radius: f32,
    pub visual_half_height: f32,
    pub camera_eye_height: f32,
    pub crouched_camera_eye_height: f32,
    pub crouch_camera_speed: f32,
    pub sprint_multiplier: f32,
    pub jump_speed: f32,
    pub gravity: f32,
    pub contact_skin: f32,
    pub ground_probe_distance: f32,
    pub max_slope_radians: f32,
    pub footstep_stride: f32,
    pub landing_speed_threshold: f32,
}

impl Default for FpsPlayerTuning {
    #[inline]
    fn default() -> Self {
        let data = default_game_data().player.tuning;
        Self {
            body_radius: data.body_radius,
            body_half_height: data.body_half_height,
            crouched_body_half_height: data.crouched_body_half_height,
            visual_radius: data.visual_radius,
            visual_half_height: data.visual_half_height,
            camera_eye_height: data.camera_eye_height,
            crouched_camera_eye_height: data.crouched_camera_eye_height,
            crouch_camera_speed: data.crouch_camera_speed,
            sprint_multiplier: data.sprint_multiplier,
            jump_speed: data.jump_speed,
            gravity: data.gravity,
            contact_skin: data.contact_skin,
            ground_probe_distance: data.ground_probe_distance,
            max_slope_radians: data.max_slope_degrees.to_radians(),
            footstep_stride: data.footstep_stride,
            landing_speed_threshold: data.landing_speed_threshold,
        }
    }
}

impl FpsPlayerTuning {
    #[inline]
    pub fn sanitized(self) -> Self {
        Self {
            body_radius: self.body_radius.clamp(0.05, 5.0),
            body_half_height: self.body_half_height.clamp(0.05, 8.0),
            crouched_body_half_height: self
                .crouched_body_half_height
                .clamp(0.05, self.body_half_height.clamp(0.05, 8.0)),
            visual_radius: self.visual_radius.clamp(0.05, 8.0),
            visual_half_height: self.visual_half_height.clamp(0.05, 12.0),
            camera_eye_height: self.camera_eye_height.clamp(0.05, 12.0),
            crouched_camera_eye_height: self.crouched_camera_eye_height.clamp(0.05, 12.0),
            crouch_camera_speed: self.crouch_camera_speed.clamp(0.1, 100.0),
            sprint_multiplier: self.sprint_multiplier.clamp(1.0, 8.0),
            jump_speed: self.jump_speed.clamp(0.0, 30.0),
            gravity: self.gravity.clamp(0.0, 80.0),
            contact_skin: self.contact_skin.clamp(0.0, 0.50),
            ground_probe_distance: self.ground_probe_distance.clamp(0.01, 2.0),
            max_slope_radians: self
                .max_slope_radians
                .clamp(0.0, core::f32::consts::FRAC_PI_2),
            footstep_stride: self.footstep_stride.clamp(0.25, 10.0),
            landing_speed_threshold: self.landing_speed_threshold.clamp(0.0, 100.0),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct FpsDemoRules {
    pub default_status: String,
    pub pickup_status: String,
    pub target_status: String,
    pub hazard_status: String,
    pub goal_locked_status: String,
    pub goal_complete_status: String,
    pub failed_progress_label: String,
    pub completed_progress_label: String,
    pub player: FpsPlayerTuning,
}

impl Default for FpsDemoRules {
    #[inline]
    fn default() -> Self {
        let status = &default_game_data().gameplay.status;
        Self {
            default_status: status.default_status.clone(),
            pickup_status: status.pickup_status.clone(),
            target_status: status.target_status.clone(),
            hazard_status: status.hazard_status.clone(),
            goal_locked_status: status.goal_locked_status.clone(),
            goal_complete_status: status.goal_complete_status.clone(),
            failed_progress_label: status.failed_progress_label.clone(),
            completed_progress_label: status.completed_progress_label.clone(),
            player: FpsPlayerTuning::default(),
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct FpsDemoPickup {
    pub radius: f32,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct FpsDemoGoal {
    pub radius: f32,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct FpsDemoHazard {
    pub radius: f32,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct FpsDemoTarget;

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub struct FpsDemoState {
    pub title: String,
    pub objective: String,
    pub elapsed_sec: f32,
    pub pickups_collected: u32,
    pub pickups_total: u32,
    pub targets_destroyed: u32,
    pub targets_total: u32,
    pub completed: bool,
    pub failed: bool,
    pub status: String,
    pub failed_progress_label: String,
    pub completed_progress_label: String,
}

#[allow(dead_code)]
impl FpsDemoState {
    #[inline]
    pub fn new(pickups_total: u32) -> Self {
        Self::from_rules(
            pickups_total,
            "KAYLA FPS: Extraction Yard",
            "Collect cores and reach the extraction beacon",
            &FpsDemoRules::default(),
        )
    }

    #[inline]
    pub fn from_rules(
        pickups_total: u32,
        title: impl Into<String>,
        objective: impl Into<String>,
        rules: &FpsDemoRules,
    ) -> Self {
        Self::from_rules_with_targets(pickups_total, 0, title, objective, rules)
    }

    #[inline]
    pub fn from_rules_with_targets(
        pickups_total: u32,
        targets_total: u32,
        title: impl Into<String>,
        objective: impl Into<String>,
        rules: &FpsDemoRules,
    ) -> Self {
        Self {
            title: title.into(),
            objective: objective.into(),
            elapsed_sec: 0.0,
            pickups_collected: 0,
            pickups_total,
            targets_destroyed: 0,
            targets_total,
            completed: false,
            failed: false,
            status: rules.default_status.clone(),
            failed_progress_label: rules.failed_progress_label.clone(),
            completed_progress_label: rules.completed_progress_label.clone(),
        }
    }

    #[inline]
    pub fn progress_label(&self) -> String {
        if self.completed {
            return format!(
                "{} in {:.1}s",
                self.completed_progress_label,
                self.elapsed_sec.max(0.0)
            );
        }
        if self.failed {
            return self.failed_progress_label.clone();
        }
        format!(
            "Cores {}/{} · Targets {}/{} · {:.1}s",
            self.pickups_collected.min(self.pickups_total),
            self.pickups_total,
            self.targets_destroyed.min(self.targets_total),
            self.targets_total,
            self.elapsed_sec.max(0.0)
        )
    }
}

impl Default for FpsDemoState {
    #[inline]
    fn default() -> Self {
        Self::new(0)
    }
}
