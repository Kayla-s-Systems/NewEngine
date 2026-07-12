use super::*;

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
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PlayerCommandFrame {
    pub source_frame: u64,
    pub actions: GameplayActionFrame,
}

impl PlayerCommandFrame {
    #[inline]
    pub const fn new(source_frame: u64, actions: GameplayActionFrame) -> Self {
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
        Self::standing(FpsPlayerTuning::default().camera_eye_height)
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
