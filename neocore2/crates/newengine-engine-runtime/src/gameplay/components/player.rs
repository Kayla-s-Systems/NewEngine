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
pub struct PlayerModelBinding {
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
    ModelBound,
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
