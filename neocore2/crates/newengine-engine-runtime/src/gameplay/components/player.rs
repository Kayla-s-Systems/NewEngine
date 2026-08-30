use super::*;

/// Provider-neutral character collision/view envelope.
///
/// This component describes physical body/eye/placeholder-visual geometry only. Gameplay
/// packages may author different values for FPS, third-person, RTS-controlled pawns, NPCs, etc.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CharacterBody {
    pub radius: f32,
    pub standing_half_height: f32,
    pub crouched_half_height: f32,
    pub standing_eye_height: f32,
    pub crouched_eye_height: f32,
    pub visual_radius: f32,
    pub visual_half_height: f32,
}

impl CharacterBody {
    #[inline]
    pub fn sanitized(self) -> Self {
        let standing_half_height = finite_or(self.standing_half_height, 0.45).clamp(0.05, 8.0);
        Self {
            radius: finite_or(self.radius, 0.45).clamp(0.05, 5.0),
            standing_half_height,
            crouched_half_height: finite_or(self.crouched_half_height, 0.15)
                .clamp(0.05, standing_half_height),
            standing_eye_height: finite_or(self.standing_eye_height, 0.72).clamp(0.05, 12.0),
            crouched_eye_height: finite_or(self.crouched_eye_height, 0.45).clamp(0.05, 12.0),
            visual_radius: finite_or(self.visual_radius, 0.45).clamp(0.05, 8.0),
            visual_half_height: finite_or(self.visual_half_height, 0.90).clamp(0.05, 12.0),
        }
    }
}

impl Default for CharacterBody {
    #[inline]
    fn default() -> Self {
        Self {
            radius: 0.45,
            standing_half_height: 0.45,
            crouched_half_height: 0.15,
            standing_eye_height: 0.72,
            crouched_eye_height: 0.45,
            visual_radius: 0.45,
            visual_half_height: 0.90,
        }
    }
}

/// Provider-neutral motion parameters consumed by generic character/player bridges.
/// Product-specific rules decide when jump/sprint/stance intents are requested.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CharacterMotionTuning {
    pub sprint_multiplier: f32,
    pub jump_speed: f32,
    pub stance_camera_speed: f32,
}

impl CharacterMotionTuning {
    #[inline]
    pub fn sanitized(self) -> Self {
        Self {
            sprint_multiplier: finite_or(self.sprint_multiplier, 1.75).clamp(1.0, 8.0),
            jump_speed: finite_or(self.jump_speed, 5.5).clamp(0.0, 30.0),
            stance_camera_speed: finite_or(self.stance_camera_speed, 12.0).clamp(0.1, 100.0),
        }
    }
}

impl Default for CharacterMotionTuning {
    #[inline]
    fn default() -> Self {
        Self {
            sprint_multiplier: 1.75,
            jump_speed: 5.5,
            stance_camera_speed: 12.0,
        }
    }
}

/// Authored locomotion targets in metres per second.
///
/// `CharacterMotor.move_speed` remains the low-level motor scalar and is projected from
/// `run`. The remaining targets are semantic authoring data used to derive stance/sprint
/// speed ratios and animation thresholds without baking product-specific constants into
/// the generic engine runtime.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlayerMovementSpeeds {
    pub walk: f32,
    pub run: f32,
    pub sprint: f32,
    pub crouch: f32,
}

impl PlayerMovementSpeeds {
    #[inline]
    pub fn sanitized(self) -> Self {
        let run = finite_or(self.run, 6.0).clamp(0.05, 50.0);
        let walk = finite_or(self.walk, run).clamp(0.05, run);
        let sprint = finite_or(self.sprint, run * 1.75).clamp(run, 75.0);
        let crouch = finite_or(self.crouch, walk).clamp(0.05, run);
        Self {
            walk,
            run,
            sprint,
            crouch,
        }
    }

    #[inline]
    pub fn sprint_multiplier(self) -> f32 {
        let value = self.sanitized();
        value.sprint / value.run
    }

    #[inline]
    pub fn crouch_multiplier(self) -> f32 {
        let value = self.sanitized();
        value.crouch / value.run
    }

    #[inline]
    pub fn walk_run_threshold(self) -> f32 {
        let value = self.sanitized();
        (value.walk + value.run) * 0.5
    }
}

impl Default for PlayerMovementSpeeds {
    #[inline]
    fn default() -> Self {
        // Preserve the pre-authored generic controller behaviour. Product profiles/YTYP
        // definitions supply character-specific values.
        Self {
            walk: 6.0,
            run: 6.0,
            sprint: 10.5,
            crouch: 6.0,
        }
    }
}

#[inline]
fn finite_or(value: f32, fallback: f32) -> f32 {
    if value.is_finite() {
        value
    } else {
        fallback
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum PlayerControllerKind {
    #[default]
    LocalInput,
    AiDriven,
    RemoteInput,
}

/// Controller marker/config attached to the same ordinary ECS entity that is
/// currently possessed by local input. The player is selected by components,
/// not by a hard-coded singleton outside ECS.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlayerController {
    pub kind: PlayerControllerKind,
    pub enabled: bool,
}

impl PlayerController {
    #[inline]
    pub const fn local_input() -> Self {
        Self {
            kind: PlayerControllerKind::LocalInput,
            enabled: true,
        }
    }
}

impl Default for PlayerController {
    #[inline]
    fn default() -> Self {
        Self::local_input()
    }
}

/// Per-render-frame semantic commands handed from input resolution to the possessed player.
/// Pulse commands are identified by `source_frame` so fixed-step systems can consume them once.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PlayerCommandFrame {
    pub source_frame: u64,
    pub actions: ActionCommandFrame,
}

impl PlayerCommandFrame {
    #[inline]
    pub fn new(source_frame: u64, actions: ActionCommandFrame) -> Self {
        Self {
            source_frame,
            actions,
        }
    }
}

/// Ground probe result owned by the player ECS entity.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlayerGroundState {
    pub grounded: bool,
    pub walkable: bool,
    pub ground_entity: Option<u64>,
    pub distance: f32,
    pub normal: Vec3,
    pub slope_radians: f32,
    pub last_fixed_tick: u64,
}

impl Default for PlayerGroundState {
    #[inline]
    fn default() -> Self {
        Self {
            grounded: false,
            walkable: false,
            ground_entity: None,
            distance: f32::INFINITY,
            normal: Vec3::Y,
            slope_radians: core::f32::consts::FRAC_PI_2,
            last_fixed_tick: 0,
        }
    }
}

impl PlayerGroundState {
    /// Number of consecutive fixed-step probe misses tolerated after a confirmed walkable hit.
    /// This is presentation/control contact hysteresis, not coyote-time: an explicit jump clears
    /// `grounded` immediately and therefore never receives this retention path.
    pub const PROBE_MISS_GRACE_TICKS: u64 = 2;

    #[inline]
    pub fn clear_for_tick(&mut self, fixed_tick: u64) {
        let contact_age = fixed_tick.saturating_sub(self.last_fixed_tick);
        if self.grounded
            && self.walkable
            && self.last_fixed_tick != 0
            && contact_age <= Self::PROBE_MISS_GRACE_TICKS
        {
            return;
        }

        self.grounded = false;
        self.walkable = false;
        self.ground_entity = None;
        self.distance = f32::INFINITY;
        self.normal = Vec3::Y;
        self.slope_radians = core::f32::consts::FRAC_PI_2;
        // `last_fixed_tick` is intentionally the last confirmed contact tick. A missing
        // probe must not masquerade as a new contact observation; valid query hits update it.
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PlayerLocomotionState {
    pub was_grounded: bool,
    pub step_distance: f32,
    pub airborne_time: f32,
    pub max_downward_speed: f32,
    /// True only when gameplay explicitly initiated a jump. Physics contact correction
    /// may create positive Y velocity on uneven terrain and must not synthesize Jump.
    pub jump_started: bool,
    /// Last input sample that consumed the edge-triggered jump command. Fixed-step catch-up
    /// may execute several ticks against one sampled command frame; the jump edge is one-shot.
    pub last_jump_command_source_frame: Option<u64>,
}

/// Continuous fall metrics owned by gameplay physics and exposed to presentation systems.
///
/// `FallStarted`/`FallEnded` player events delimit the lifetime of a fall. Animation, camera,
/// VFX or audio subscribers can then read this component every frame and select authored
/// presentation from the actual world-space fall distance instead of hard-coding clip thresholds
/// in the character controller. `peak_height` tracks the airborne apex, so jumps measure the
/// downward part of the trajectory rather than the initial take-off height.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PlayerFallState {
    pub airborne: bool,
    pub falling: bool,
    pub start_height: f32,
    pub peak_height: f32,
    pub current_height: f32,
    pub distance: f32,
    pub max_distance: f32,
    pub downward_speed: f32,
    pub revision: u64,
}

/// Last resolved landing impact. Unlike `PlayerFallState`, this survives the grounded transition so
/// presentation systems can consume the impact on the following render/animation frame.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PlayerLandingState {
    pub distance: f32,
    pub downward_speed: f32,
    pub horizontal_speed: f32,
    pub revision: u64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PlayerLocomotionAnimation {
    #[default]
    Idle,
    Walk,
    Run,
    Sprint,
    CrouchIdle,
    CrouchWalk,
    Jump,
    Fall,
}

impl PlayerLocomotionAnimation {
    #[inline]
    pub const fn clip_hint(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Walk => "walk",
            Self::Run => "run",
            Self::Sprint => "sprint",
            Self::CrouchIdle => "crouch_idle",
            Self::CrouchWalk => "crouch_walk",
            Self::Jump => "jump",
            Self::Fall => "fall",
        }
    }
}

/// Engine-owned locomotion animation state. This is intentionally clip-format neutral:
/// animation providers may map the semantic state to YCD clips, blend trees, motion
/// matching, or another backend without changing PlayerActor/controller code.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlayerAnimationState {
    pub locomotion: PlayerLocomotionAnimation,
    pub normalized_speed: f32,
    pub cycle_phase: f32,
    pub transition_alpha: f32,
    pub revision: u64,
}

impl Default for PlayerAnimationState {
    #[inline]
    fn default() -> Self {
        Self {
            locomotion: PlayerLocomotionAnimation::Idle,
            normalized_speed: 0.0,
            cycle_phase: 0.0,
            transition_alpha: 1.0,
            revision: 1,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PlayerStanceKind {
    #[default]
    Standing,
    Crouched,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlayerStanceState {
    pub current: PlayerStanceKind,
    pub stand_requested: bool,
    pub stand_blocked: bool,
    pub current_eye_height: f32,
    pub target_eye_height: f32,
    pub last_transition_tick: u64,
}

impl PlayerStanceState {
    #[inline]
    pub const fn standing(eye_height: f32) -> Self {
        Self {
            current: PlayerStanceKind::Standing,
            stand_requested: false,
            stand_blocked: false,
            current_eye_height: eye_height,
            target_eye_height: eye_height,
            last_transition_tick: 0,
        }
    }
}

impl Default for PlayerStanceState {
    fn default() -> Self {
        Self::standing(CharacterBody::default().standing_eye_height)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlayerJointRotationWeight {
    pub joint: String,
    pub weight: f32,
    pub channels: PlayerJointChannels,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlayerJointChannels {
    pub translation: bool,
    pub rotation: bool,
    pub scale: bool,
}

impl PlayerJointChannels {
    #[inline]
    pub const fn rotation_only() -> Self {
        Self {
            translation: false,
            rotation: true,
            scale: false,
        }
    }

    #[inline]
    pub const fn translation_rotation() -> Self {
        Self {
            translation: true,
            rotation: true,
            scale: false,
        }
    }

    #[inline]
    pub const fn all() -> Self {
        Self {
            translation: true,
            rotation: true,
            scale: true,
        }
    }

    #[inline]
    pub const fn any(self) -> bool {
        self.translation || self.rotation || self.scale
    }
}

impl Default for PlayerJointChannels {
    fn default() -> Self {
        Self::rotation_only()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlayerJointCopyRule {
    pub source_joint: String,
    pub target_joint: String,
    pub channels: PlayerJointChannels,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlayerPaletteFollowRule {
    pub driver_joint: String,
    pub follower_roots: Vec<String>,
    pub include_descendants: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlayerEyeParentFollowRule {
    pub left_joint: String,
    pub right_joint: String,
    pub parent_joint: String,
    pub preserve_bind_local: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlayerWeaponArmIkRigDefinition {
    pub chest: String,
    pub right_shoulder: String,
    pub right_elbow: String,
    pub right_wrist: String,
    pub right_palm: String,
    pub right_prop_attachment: Option<String>,
    pub left_shoulder: String,
    pub left_elbow: String,
    pub left_wrist: String,
    pub left_palm: String,
    pub left_prop_attachment: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct PlayerBraidSecondaryMotionRig {
    pub chain_joints: Vec<String>,
    pub head_joint: String,
    pub head_base_joint: String,
    pub upper_back_joint: String,
    pub middle_back_joint: String,
    pub lower_back_joint: String,
    pub left_shoulder_joint: String,
    pub right_shoulder_joint: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlayerCharacterPresentation {
    pub detached_head_follow: bool,
    pub detached_head_follow_rule: Option<PlayerPaletteFollowRule>,
    pub eye_parent_follow: bool,
    pub eye_parent_follow_rule: Option<PlayerEyeParentFollowRule>,
    pub helper_pose_copies: Vec<PlayerJointCopyRule>,
    pub braid_secondary_motion: Option<PlayerBraidSecondaryMotionRig>,
    pub equipment_ready_animation: Option<String>,
    pub equipment_aim_animation: Option<String>,
    pub equipment_reload_animation: Option<String>,
    pub unarmed_ready_animation: Option<String>,
    pub unarmed_attack_animation: Option<String>,
    pub noclip_animation: Option<String>,
    /// Optional authored turn-in-place clips. These are full-body steps; stationary mouse yaw never
    /// rotates the world root directly. Runtime selects the nearest signed angle.
    pub turn_45_left_animation: Option<String>,
    pub turn_45_right_animation: Option<String>,
    pub turn_90_left_animation: Option<String>,
    pub turn_90_right_animation: Option<String>,
    pub turn_135_left_animation: Option<String>,
    pub turn_135_right_animation: Option<String>,
    pub turn_180_left_animation: Option<String>,
    pub turn_180_right_animation: Option<String>,
    /// Optional full-body fall clips selected from the runtime fall-distance signal.
    pub fall_low_animation: Option<String>,
    pub fall_medium_animation: Option<String>,
    pub fall_high_animation: Option<String>,
    /// Optional character-native landing response clips. These run after `FallEnded`, not while airborne.
    pub landing_soft_animation: Option<String>,
    pub landing_medium_animation: Option<String>,
    pub landing_hard_animation: Option<String>,
    pub landing_hard_run_animation: Option<String>,
    /// Project-authored distance thresholds measured from the airborne trajectory apex.
    pub fall_medium_min_distance: f32,
    pub fall_high_min_distance: f32,
    pub equipment_ready_sample_phase: f32,
    pub equipment_ready_rotation_weights: Vec<PlayerJointRotationWeight>,
    pub equipment_aim_rotation_weights: Vec<PlayerJointRotationWeight>,
    pub equipment_reload_rotation_weights: Vec<PlayerJointRotationWeight>,
    pub equipment_arm_ik: bool,
    pub equipment_arm_ik_rig: Option<PlayerWeaponArmIkRigDefinition>,
}

impl Default for PlayerCharacterPresentation {
    fn default() -> Self {
        Self {
            detached_head_follow: false,
            detached_head_follow_rule: None,
            eye_parent_follow: false,
            eye_parent_follow_rule: None,
            helper_pose_copies: Vec::new(),
            braid_secondary_motion: None,
            equipment_ready_animation: None,
            equipment_aim_animation: None,
            equipment_reload_animation: None,
            unarmed_ready_animation: None,
            unarmed_attack_animation: None,
            noclip_animation: None,
            turn_45_left_animation: None,
            turn_45_right_animation: None,
            turn_90_left_animation: None,
            turn_90_right_animation: None,
            turn_135_left_animation: None,
            turn_135_right_animation: None,
            turn_180_left_animation: None,
            turn_180_right_animation: None,
            fall_low_animation: None,
            fall_medium_animation: None,
            fall_high_animation: None,
            landing_soft_animation: None,
            landing_medium_animation: None,
            landing_hard_animation: None,
            landing_hard_run_animation: None,
            fall_medium_min_distance: 0.0,
            fall_high_min_distance: 0.0,
            equipment_ready_sample_phase: 0.0,
            equipment_ready_rotation_weights: Vec::new(),
            equipment_aim_rotation_weights: Vec::new(),
            equipment_reload_rotation_weights: Vec::new(),
            equipment_arm_ik: false,
            equipment_arm_ik_rig: None,
        }
    }
}

/// Desired player avatar assignment. Game/editor code changes this component;
/// the active world package resolves it to a concrete runtime model binding.
#[derive(Clone, Debug, PartialEq)]
pub struct PlayerModelAssignment {
    pub revision: u64,
    pub enabled: bool,
    pub source: String,
    pub properties_ref: Option<String>,
    pub texture_dictionary: Option<String>,
    pub skeleton_source: Option<String>,
    /// Semantic idle clip reference, e.g. `animations/foo.ycd@idle`.
    pub idle_animation: Option<String>,
    pub walk_animation: Option<String>,
    pub run_animation: Option<String>,
    pub sprint_animation: Option<String>,
    pub crouch_idle_animation: Option<String>,
    pub crouch_walk_animation: Option<String>,
    pub jump_animation: Option<String>,
    pub fall_animation: Option<String>,
    pub presentation: PlayerCharacterPresentation,
    pub target_height: f32,
    pub eye_height_ratio: f32,
    pub local_offset: Vec3,
    pub yaw_offset: f32,
    pub hide_in_first_person: bool,
}

impl PlayerModelAssignment {
    #[inline]
    pub fn new(source: impl Into<String>) -> Self {
        Self {
            revision: 1,
            enabled: true,
            source: source.into(),
            ..Self::default()
        }
    }

    #[inline]
    pub fn next_revision_after(mut self, previous: Option<&Self>) -> Self {
        self.revision = previous
            .map(|assignment| assignment.revision.saturating_add(1).max(1))
            .unwrap_or_else(|| self.revision.max(1));
        self
    }
}

impl Default for PlayerModelAssignment {
    #[inline]
    fn default() -> Self {
        Self {
            revision: 0,
            enabled: false,
            source: String::new(),
            properties_ref: None,
            texture_dictionary: None,
            skeleton_source: None,
            idle_animation: None,
            walk_animation: None,
            run_animation: None,
            sprint_animation: None,
            crouch_idle_animation: None,
            crouch_walk_animation: None,
            jump_animation: None,
            fall_animation: None,
            presentation: PlayerCharacterPresentation::default(),
            target_height: 1.80,
            eye_height_ratio: 0.91,
            local_offset: Vec3::ZERO,
            yaw_offset: 0.0,
            hide_in_first_person: false,
        }
    }
}

/// Two authoritative fixed-step poses retained for render interpolation.
/// Simulation/physics continue to own `Transform`; this component is presentation history only.
#[derive(Clone, Copy, Debug)]
pub struct PlayerFixedPoseHistory {
    pub previous_position: Vec3,
    pub previous_rotation: newengine_math::Quat,
    pub current_position: Vec3,
    pub current_rotation: newengine_math::Quat,
    pub current_fixed_tick: u64,
    pub initialized: bool,
}

impl Default for PlayerFixedPoseHistory {
    fn default() -> Self {
        Self {
            previous_position: Vec3::ZERO,
            previous_rotation: newengine_math::Quat::IDENTITY,
            current_position: Vec3::ZERO,
            current_rotation: newengine_math::Quat::IDENTITY,
            current_fixed_tick: 0,
            initialized: false,
        }
    }
}

/// Render-cadence player pose sampled between the two latest fixed simulation poses.
/// Camera and player visuals consume the same value so third-person framing cannot jitter.
#[derive(Clone, Copy, Debug)]
pub struct PlayerRenderPose {
    pub position: Vec3,
    pub rotation: newengine_math::Quat,
    pub simulation_position: Vec3,
    pub simulation_rotation: newengine_math::Quat,
    pub fixed_alpha: f32,
    pub source_fixed_tick: u64,
}

impl Default for PlayerRenderPose {
    fn default() -> Self {
        Self {
            position: Vec3::ZERO,
            rotation: newengine_math::Quat::IDENTITY,
            simulation_position: Vec3::ZERO,
            simulation_rotation: newengine_math::Quat::IDENTITY,
            fixed_alpha: 0.0,
            source_fixed_tick: 0,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlayerModelBinding {
    pub assignment_revision: u64,
    pub source: String,
    pub skeleton_source: Option<String>,
    pub visual_root: Option<newengine_ecs::EntityId>,
    pub part_count: u32,
    pub target_height: f32,
    pub feet_to_eye_height: f32,
}

impl Default for PlayerModelBinding {
    #[inline]
    fn default() -> Self {
        Self {
            assignment_revision: 0,
            source: String::new(),
            skeleton_source: None,
            visual_root: None,
            part_count: 0,
            target_height: 1.80,
            feet_to_eye_height: 1.64,
        }
    }
}

/// Provider-neutral first-person camera anchor published by the active avatar/runtime model.
/// The position is a stable render-cadence eye center in world space. Camera orientation remains
/// input-owned (CharacterMotor yaw/pitch); animation may affect presentation but never owns the
/// gameplay camera position.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlayerFirstPersonCameraAnchor {
    pub eye_center_ws: Vec3,
    /// Small body-forward clearance from the stable eye center. View yaw/pitch must not rotate this
    /// offset around the head; camera runtime may add only bounded FPP parallax.
    pub forward_clearance: f32,
}

impl Default for PlayerFirstPersonCameraAnchor {
    #[inline]
    fn default() -> Self {
        Self {
            eye_center_ws: Vec3::ZERO,
            forward_clearance: 0.045,
        }
    }
}

/// Authorable self-collision envelope for a local first-person camera. Offsets are expressed
/// in the player body frame relative to the stable eye anchor (engine forward = -Z). The camera
/// runtime consumes only these analytic primitives; it never performs triangle collision against
/// a deforming character mesh.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlayerFirstPersonBodyBarrierProfile {
    pub enabled: bool,
    pub head_center_offset_ls: Vec3,
    pub head_radius: f32,
    pub neck_top_offset_ls: Vec3,
    pub neck_bottom_offset_ls: Vec3,
    pub neck_radius: f32,
    pub chest_top_offset_ls: Vec3,
    pub chest_bottom_offset_ls: Vec3,
    pub chest_radius: f32,
    pub surface_padding: f32,
    /// Maximum downward view pitch from the horizon. Upward pitch keeps the motor-authored limit.
    pub downward_pitch_limit_radians: f32,
}

impl PlayerFirstPersonBodyBarrierProfile {
    #[inline]
    pub fn from_body(body: CharacterBody) -> Self {
        let body = body.sanitized();
        let height_scale = (body.visual_half_height / 0.90).clamp(0.65, 1.60);
        let radius = body.visual_radius;
        Self {
            enabled: true,
            // +Z is behind the eyes. Keeping the primitive centre slightly behind the anchor
            // makes the safe projection resolve toward the face/front, not through the skull.
            head_center_offset_ls: Vec3::new(0.0, -0.035 * height_scale, 0.035 * height_scale),
            head_radius: (radius * 0.28).clamp(0.095, 0.145),
            neck_top_offset_ls: Vec3::new(0.0, -0.115 * height_scale, 0.030 * height_scale),
            neck_bottom_offset_ls: Vec3::new(0.0, -0.245 * height_scale, 0.045 * height_scale),
            neck_radius: (radius * 0.20).clamp(0.065, 0.100),
            chest_top_offset_ls: Vec3::new(0.0, -0.275 * height_scale, 0.055 * height_scale),
            chest_bottom_offset_ls: Vec3::new(0.0, -0.525 * height_scale, 0.070 * height_scale),
            chest_radius: (radius * 0.38).clamp(0.140, 0.205),
            surface_padding: 0.012,
            downward_pitch_limit_radians: 75.0_f32.to_radians(),
        }
    }

    #[inline]
    pub fn sanitized(self, fallback_body: CharacterBody) -> Self {
        let fallback = Self::from_body(fallback_body);
        let finite_vec = |value: Vec3, default: Vec3| {
            if value.is_finite() {
                value
            } else {
                default
            }
        };
        let finite_radius = |value: f32, default: f32, lo: f32, hi: f32| {
            if value.is_finite() {
                value.clamp(lo, hi)
            } else {
                default
            }
        };
        Self {
            enabled: self.enabled,
            head_center_offset_ls: finite_vec(
                self.head_center_offset_ls,
                fallback.head_center_offset_ls,
            ),
            head_radius: finite_radius(self.head_radius, fallback.head_radius, 0.04, 0.40),
            neck_top_offset_ls: finite_vec(self.neck_top_offset_ls, fallback.neck_top_offset_ls),
            neck_bottom_offset_ls: finite_vec(
                self.neck_bottom_offset_ls,
                fallback.neck_bottom_offset_ls,
            ),
            neck_radius: finite_radius(self.neck_radius, fallback.neck_radius, 0.03, 0.30),
            chest_top_offset_ls: finite_vec(self.chest_top_offset_ls, fallback.chest_top_offset_ls),
            chest_bottom_offset_ls: finite_vec(
                self.chest_bottom_offset_ls,
                fallback.chest_bottom_offset_ls,
            ),
            chest_radius: finite_radius(self.chest_radius, fallback.chest_radius, 0.05, 0.45),
            surface_padding: finite_radius(
                self.surface_padding,
                fallback.surface_padding,
                0.0,
                0.05,
            ),
            downward_pitch_limit_radians: finite_radius(
                self.downward_pitch_limit_radians,
                fallback.downward_pitch_limit_radians,
                35.0_f32.to_radians(),
                85.0_f32.to_radians(),
            ),
        }
    }
}

impl Default for PlayerFirstPersonBodyBarrierProfile {
    #[inline]
    fn default() -> Self {
        Self::from_body(CharacterBody::default())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum PlayerVisualKind {
    #[default]
    RuntimeModelPart,
    FallbackCapsule,
    EquippedWeapon,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlayerVisualPart {
    pub owner: newengine_ecs::EntityId,
    pub part_index: u32,
    pub kind: PlayerVisualKind,
    pub material_slot: String,
}

/// Eight-influence linear blend skinning vertex payload owned by the engine runtime.
/// Joint indices address the stable authored skeleton joint table. The first quartet
/// is backward-compatible with YDD V3; the second is populated by YDD V4 sources.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlayerSkinVertex {
    pub joints: [u16; 4],
    pub weights: [f32; 4],
    pub joints_extra: [u16; 4],
    pub weights_extra: [f32; 4],
}

/// Skin stream attached to one runtime player visual part. The owner points at the
/// PlayerActor that carries the current palette; source_to_model is retained for
/// diagnostics/validation and must match the pose binding used to build the palette.
#[derive(Clone, Debug, PartialEq)]
pub struct PlayerSkinBinding {
    pub owner: newengine_ecs::EntityId,
    pub vertices: Vec<PlayerSkinVertex>,
    pub source_to_model: [f32; 16],
}

/// Per-player matrix palette produced once per frame by the animation backend and
/// consumed by every skinned visual part owned by that player.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct PlayerSkinPose {
    pub palette: Vec<newengine_math::Mat4>,
    pub revision: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum PlayerViewVisibilityPolicy {
    #[default]
    AlwaysVisible,
    HideInFirstPerson,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlayerViewVisibility {
    pub base_mode: DisplayMode,
    pub policy: PlayerViewVisibilityPolicy,
}

/// Presentation signal published by the camera gateway for systems that need to distinguish
/// first-person view-model presentation from world/third-person attachment. This deliberately
/// carries no camera implementation types across the gameplay boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlayerViewState {
    pub first_person_active: bool,
}

impl Default for PlayerViewState {
    #[inline]
    fn default() -> Self {
        // CameraViewMode defaults to FirstPerson, so startup presentation must agree before the
        // first camera-gateway frame publishes an explicit state.
        Self {
            first_person_active: true,
        }
    }
}

impl PlayerViewVisibility {
    #[inline]
    pub const fn runtime_model_default() -> Self {
        Self {
            base_mode: DisplayMode::GameOnly,
            policy: PlayerViewVisibilityPolicy::AlwaysVisible,
        }
    }

    #[inline]
    pub const fn fallback_capsule_default() -> Self {
        Self {
            base_mode: DisplayMode::GameOnly,
            policy: PlayerViewVisibilityPolicy::HideInFirstPerson,
        }
    }
}

impl Default for PlayerViewVisibility {
    #[inline]
    fn default() -> Self {
        Self::runtime_model_default()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlayerEventKind {
    Spawned,
    ModelAssignmentChanged,
    ModelBound,
    AnimationStateChanged,
    Possessed,
    Released,
    InputApplied,
    GroundStateChanged,
    FallStarted,
    FallEnded,
    Footstep,
    Landed,
    StanceChanged,
    StanceBlocked,
    VisualVisibilityChanged,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlayerEvent {
    pub entity: newengine_ecs::EntityId,
    pub kind: PlayerEventKind,
    pub message: String,
}

#[derive(Clone, Debug, Default)]
pub struct PlayerEventBus {
    pub events: Vec<PlayerEvent>,
}

impl PlayerEventBus {
    #[inline]
    pub fn emit(
        &mut self,
        entity: newengine_ecs::EntityId,
        kind: PlayerEventKind,
        message: impl Into<String>,
    ) {
        const MAX_RETAINED_EVENTS: usize = 256;
        if self.events.len() >= MAX_RETAINED_EVENTS {
            let overflow = self.events.len() + 1 - MAX_RETAINED_EVENTS;
            self.events.drain(0..overflow);
        }
        self.events.push(PlayerEvent {
            entity,
            kind,
            message: message.into(),
        });
    }

    #[inline]
    pub fn drain(&mut self) -> Vec<PlayerEvent> {
        std::mem::take(&mut self.events)
    }
}
