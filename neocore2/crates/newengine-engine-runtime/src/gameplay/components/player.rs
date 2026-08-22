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
    #[inline]
    pub fn clear_for_tick(&mut self, fixed_tick: u64) {
        self.grounded = false;
        self.walkable = false;
        self.ground_entity = None;
        self.distance = f32::INFINITY;
        self.normal = Vec3::Y;
        self.slope_radians = core::f32::consts::FRAC_PI_2;
        self.last_fixed_tick = fixed_tick;
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PlayerLocomotionState {
    pub was_grounded: bool,
    pub step_distance: f32,
    pub airborne_time: f32,
    pub max_downward_speed: f32,
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
    pub jump_animation: Option<String>,
    pub fall_animation: Option<String>,
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
            jump_animation: None,
            fall_animation: None,
            target_height: 1.80,
            eye_height_ratio: 0.91,
            local_offset: Vec3::ZERO,
            yaw_offset: 0.0,
            hide_in_first_person: true,
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum PlayerVisualKind {
    #[default]
    RuntimeModelPart,
    FallbackCapsule,
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
    AlwaysVisible,
    #[default]
    HideInFirstPerson,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlayerViewVisibility {
    pub base_mode: DisplayMode,
    pub policy: PlayerViewVisibilityPolicy,
}

impl PlayerViewVisibility {
    #[inline]
    pub const fn runtime_model_default() -> Self {
        Self {
            base_mode: DisplayMode::GameOnly,
            policy: PlayerViewVisibilityPolicy::HideInFirstPerson,
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
