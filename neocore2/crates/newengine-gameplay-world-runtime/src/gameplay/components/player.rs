use super::*;

/// Provider-neutral character collision/view envelope.
///
/// This component describes physical body/eye/placeholder-visual geometry only. Gameplay
/// packages may author different values for FPS, third-person, RTS-controlled pawns, NPCs, etc.
mod camera;
pub use camera::*;

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

/// Project-authored startup view selected by the active camera definition. Runtime input may
/// subsequently switch modes, but bootstrap never invents the initial gameplay view.
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

/// Explicit authored look-at presentation context.
///
/// `Standard` delegates to ordinary locomotion/equipment state selection. Contextual gameplay
/// systems (cover, traversal, injury, etc.) may set a specific variant when they own that state.
/// Animation runtime treats this as semantic intent only: it never infers a context from clip names
/// or asset paths, and a character without an authored range for that context fails closed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum PlayerLookContext {
    #[default]
    Standard,
    CoverLowLeft,
    CoverLowRight,
    Prone,
    Supine,
    Rope,
    Ladder,
    SwimIdle,
    Injured,
    RelaxedInjured,
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
    /// Horizontal world-space velocity captured at explicit jump takeoff. FPS character policy
    /// uses it as momentum fallback while airborne so a transient zero movement sample cannot
    /// freeze the character in mid-jump. New non-zero air input remains authoritative.
    pub jump_takeoff_horizontal_velocity: Vec3,
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

include!("player/presentation.rs");
