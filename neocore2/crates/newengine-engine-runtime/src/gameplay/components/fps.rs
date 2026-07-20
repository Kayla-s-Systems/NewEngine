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
        Self {
            body_radius: 0.45,
            body_half_height: 0.45,
            crouched_body_half_height: 0.15,
            visual_radius: 0.45,
            visual_half_height: 0.90,
            camera_eye_height: 0.72,
            crouched_camera_eye_height: 0.45,
            crouch_camera_speed: 12.0,
            sprint_multiplier: 1.75,
            jump_speed: 5.5,
            gravity: 9.81,
            contact_skin: 0.035,
            ground_probe_distance: 0.25,
            max_slope_radians: 50.0_f32.to_radians(),
            footstep_stride: 2.1,
            landing_speed_threshold: 3.0,
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
        Self {
            default_status:
                "Collect field cores, neutralize targets, avoid hazards, reach extraction."
                    .to_string(),
            pickup_status: "Core acquired.".to_string(),
            target_status: "Target neutralized.".to_string(),
            hazard_status: "You touched a hazard. Relaunch the demo to retry.".to_string(),
            goal_locked_status: "Beacon locked: collect all cores first.".to_string(),
            goal_complete_status: "Extraction complete. Stable runtime loop is playable."
                .to_string(),
            failed_progress_label: "FAILED — touch a hazard to retry scene".to_string(),
            completed_progress_label: "EXTRACTED".to_string(),
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

/// Runtime launch gate for standalone game-ready scenes.
///
/// Scene bootstrap may finish on the CPU before renderer-owned resources
/// become resident on the GPU. The render controller owns the final release
/// decision and keeps direct player control/physics closed until this gate is
/// released.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GameReadyWorldLaunchGatePhase {
    WaitingForResidency,
    Released,
    /// Renderer/world resources are resident and the editor may show a preview,
    /// but simulation and direct player control remain stopped until explicit
    /// Simulate or Play In Editor intent.
    EditorPreviewReady,
    PlayActivated,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GameReadyWorldLaunchGate {
    pub requested_frame: u64,
    /// Process-monotonic origin for the soft launch timeout. Frame-based timeouts
    /// alone are insufficient while prelaunch GPU uploads make frames uneven; a
    /// monotonic clock also avoids system-time adjustments extending the gate.
    pub requested_at_ms: u64,
    pub released_frame: Option<u64>,
    pub phase: GameReadyWorldLaunchGatePhase,
    pub reason: String,
    pub waiting_textures: u32,
    pub total_textures: u32,
    pub failed_textures: u32,
}

impl GameReadyWorldLaunchGate {
    #[inline]
    pub fn new(reason: impl Into<String>) -> Self {
        Self {
            requested_frame: u64::MAX,
            requested_at_ms: 0,
            released_frame: None,
            phase: GameReadyWorldLaunchGatePhase::WaitingForResidency,
            reason: reason.into(),
            waiting_textures: 0,
            total_textures: 0,
            failed_textures: 0,
        }
    }

    #[inline]
    pub const fn is_released(&self) -> bool {
        matches!(
            self.phase,
            GameReadyWorldLaunchGatePhase::Released
                | GameReadyWorldLaunchGatePhase::EditorPreviewReady
                | GameReadyWorldLaunchGatePhase::PlayActivated
        )
    }

    #[inline]
    pub const fn is_play_activated(&self) -> bool {
        matches!(self.phase, GameReadyWorldLaunchGatePhase::PlayActivated)
    }

    #[inline]
    pub const fn is_editor_preview_ready(&self) -> bool {
        matches!(
            self.phase,
            GameReadyWorldLaunchGatePhase::EditorPreviewReady
        )
    }

    #[inline]
    pub const fn needs_prelaunch_gate(&self) -> bool {
        matches!(
            self.phase,
            GameReadyWorldLaunchGatePhase::WaitingForResidency
                | GameReadyWorldLaunchGatePhase::Released
        )
    }

    #[inline]
    pub fn release(&mut self, frame: u64, reason: impl Into<String>) {
        self.mark_ready(frame, GameReadyWorldLaunchGatePhase::Released, reason);
    }

    #[inline]
    pub fn update_texture_counts(&mut self, waiting: u32, total: u32, failed: u32) {
        self.waiting_textures = waiting;
        self.total_textures = total;
        self.failed_textures = failed;
    }

    #[inline]
    pub fn mark_editor_preview_ready(&mut self, frame: u64, reason: impl Into<String>) {
        self.mark_ready(
            frame,
            GameReadyWorldLaunchGatePhase::EditorPreviewReady,
            reason,
        );
    }

    fn mark_ready(
        &mut self,
        frame: u64,
        phase: GameReadyWorldLaunchGatePhase,
        reason: impl Into<String>,
    ) {
        self.requested_frame = self.requested_frame.min(frame);
        self.released_frame = Some(frame);
        self.phase = phase;
        self.reason = reason.into();
        self.waiting_textures = 0;
    }

    #[inline]
    pub fn mark_play_activated(&mut self) {
        self.phase = GameReadyWorldLaunchGatePhase::PlayActivated;
    }
}
