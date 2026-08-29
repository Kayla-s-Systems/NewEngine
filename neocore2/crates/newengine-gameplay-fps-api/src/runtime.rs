#![forbid(unsafe_op_in_unsafe_fn)]

/// Authored spring/K response model carried into the FPS runtime.
///
/// This is a data contract only. In particular, `max_accel = -1` is preserved verbatim; the
/// original native sentinel semantics are intentionally not guessed here.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FpsMotionResponseTuning {
    pub velocity_spring_const: f32,
    pub velocity_spring_const_decel: f32,
    pub velocity_spring_dampen_ratio: f32,
    pub speed_spring_const: f32,
    pub max_accel: f32,
    pub trans_clamp_dist: f32,
}

impl FpsMotionResponseTuning {
    #[inline]
    pub fn sanitized(self) -> Option<Self> {
        let values = [
            self.velocity_spring_const,
            self.velocity_spring_const_decel,
            self.velocity_spring_dampen_ratio,
            self.speed_spring_const,
            self.max_accel,
            self.trans_clamp_dist,
        ];
        if values.iter().any(|value| !value.is_finite()) {
            return None;
        }
        // Preserve authored values exactly. The original domain/sentinel semantics are not fully
        // recovered yet, so this layer deliberately does not invent numeric bounds.
        Some(self)
    }
}

/// Declarative FPS runtime tuning.
///
/// The scene/profile owns these values; runtime systems only consume the resource.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FpsPlayerTuning {
    pub motion_response: Option<FpsMotionResponseTuning>,
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
    pub locomotion_min_horizontal_speed: f32,
    pub ground_probe_max_upward_velocity: f32,
    pub landing_min_airborne_seconds: f32,
}

impl Default for FpsPlayerTuning {
    #[inline]
    fn default() -> Self {
        // Mechanics-safe schema defaults only. Product/player tuning is authored by the
        // active project and installed as `FpsDemoRules` during scene bootstrap.
        Self {
            motion_response: None,
            body_radius: 0.35,
            body_half_height: 0.55,
            crouched_body_half_height: 0.30,
            visual_radius: 0.35,
            visual_half_height: 0.90,
            camera_eye_height: 0.62,
            crouched_camera_eye_height: 0.35,
            crouch_camera_speed: 8.0,
            sprint_multiplier: 1.5,
            jump_speed: 5.0,
            gravity: 9.81,
            contact_skin: 0.03,
            ground_probe_distance: 0.12,
            max_slope_radians: core::f32::consts::FRAC_PI_4,
            footstep_stride: 1.4,
            landing_speed_threshold: 4.0,
            locomotion_min_horizontal_speed: 0.15,
            ground_probe_max_upward_velocity: 0.1,
            landing_min_airborne_seconds: 0.05,
        }
    }
}

impl FpsPlayerTuning {
    #[inline]
    pub fn sanitized(self) -> Self {
        Self {
            motion_response: self
                .motion_response
                .and_then(FpsMotionResponseTuning::sanitized),
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
            locomotion_min_horizontal_speed: self.locomotion_min_horizontal_speed.clamp(0.0, 20.0),
            ground_probe_max_upward_velocity: self
                .ground_probe_max_upward_velocity
                .clamp(-20.0, 20.0),
            landing_min_airborne_seconds: self.landing_min_airborne_seconds.clamp(0.0, 5.0),
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
        Self {
            default_status: String::new(),
            pickup_status: String::new(),
            target_status: String::new(),
            hazard_status: String::new(),
            goal_locked_status: String::new(),
            goal_complete_status: String::new(),
            failed_progress_label: String::new(),
            completed_progress_label: String::new(),
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

/// Persistent physical brass ejected by an FPS weapon shot. The gameplay layer owns motion and
/// collision; product world packages may attach an authored visual selected by `variant`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WeaponShellCasing {
    pub owner_stable_id: u64,
    pub shot_sequence: u64,
    /// Stable `ItemId::raw()` of the weapon whose authored definition owns this casing.
    pub weapon_item_id: u64,
    /// Zero-based authored variant index resolved from the equipped weapon definition.
    pub variant: u16,
}

impl WeaponShellCasing {
    #[inline]
    pub const fn new(
        owner_stable_id: u64,
        shot_sequence: u64,
        weapon_item_id: u64,
        variant: u16,
    ) -> Self {
        Self {
            owner_stable_id,
            shot_sequence,
            weapon_item_id,
            variant,
        }
    }
}

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
            String::new(),
            String::new(),
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

#[cfg(test)]
mod motion_response_tests {
    use super::*;

    #[test]
    fn authored_motion_response_survives_runtime_sanitization_verbatim() {
        let authored = FpsMotionResponseTuning {
            velocity_spring_const: 7.0,
            velocity_spring_const_decel: 10.0,
            velocity_spring_dampen_ratio: 1.0,
            speed_spring_const: 4.6,
            max_accel: -1.0,
            trans_clamp_dist: 0.01,
        };
        assert_eq!(authored.sanitized(), Some(authored));

        let tuning = FpsPlayerTuning {
            motion_response: Some(authored),
            ..FpsPlayerTuning::default()
        }
        .sanitized();
        assert_eq!(tuning.motion_response, Some(authored));
    }

    #[test]
    fn non_finite_motion_response_is_rejected_without_rewriting_other_values() {
        let invalid = FpsMotionResponseTuning {
            velocity_spring_const: 7.0,
            velocity_spring_const_decel: 10.0,
            velocity_spring_dampen_ratio: 1.0,
            speed_spring_const: 4.6,
            max_accel: f32::NAN,
            trans_clamp_dist: 0.01,
        };
        assert_eq!(invalid.sanitized(), None);
    }

    #[test]
    fn generic_runtime_does_not_invent_a_motion_response() {
        assert_eq!(FpsPlayerTuning::default().motion_response, None);
    }
}
