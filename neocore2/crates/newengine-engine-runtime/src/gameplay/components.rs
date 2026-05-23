pub use newengine_physics_contracts::{CollisionShapeDesc, PhysicsBodyDesc};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum GameRunMode {
    #[default]
    Staging,
    Simulate,
    Play,
}

impl GameRunMode {
    #[inline]
    pub const fn is_runtime(self) -> bool {
        matches!(self, Self::Simulate | Self::Play)
    }

    #[inline]
    pub const fn runs_physics(self) -> bool {
        self.is_runtime()
    }

    #[inline]
    pub const fn wants_direct_player_control(self) -> bool {
        matches!(self, Self::Play)
    }
}

// Physics components are owned by `newengine-physics-contracts`.
// GameFirst runtime may re-export them for callers, but gameplay code must not
// define its own collision model or store backend-native handles.

#[derive(Clone, Copy, Debug, Default)]
pub struct PlayerActor;

#[derive(Clone, Copy, Debug, Default)]
pub struct GameplayActor;

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
    pub fn emit(&mut self, entity: newengine_ecs::EntityId, kind: PlayerEventKind, message: impl Into<String>) {
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

/// Declarative FPS runtime tuning.
///
/// The scene/profile owns these values; runtime systems only consume the resource.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FpsPlayerTuning {
    pub body_radius: f32,
    pub body_half_height: f32,
    pub visual_radius: f32,
    pub visual_half_height: f32,
    pub camera_eye_height: f32,
    pub sprint_multiplier: f32,
    pub gravity: f32,
    pub contact_skin: f32,
}

impl Default for FpsPlayerTuning {
    #[inline]
    fn default() -> Self {
        Self {
            body_radius: 0.45,
            body_half_height: 0.45,
            visual_radius: 0.45,
            visual_half_height: 0.90,
            camera_eye_height: 0.72,
            sprint_multiplier: 1.75,
            gravity: 9.81,
            contact_skin: 0.035,
        }
    }
}

impl FpsPlayerTuning {
    #[inline]
    pub fn sanitized(self) -> Self {
        Self {
            body_radius: self.body_radius.clamp(0.05, 5.0),
            body_half_height: self.body_half_height.clamp(0.05, 8.0),
            visual_radius: self.visual_radius.clamp(0.05, 8.0),
            visual_half_height: self.visual_half_height.clamp(0.05, 12.0),
            camera_eye_height: self.camera_eye_height.clamp(0.05, 12.0),
            sprint_multiplier: self.sprint_multiplier.clamp(1.0, 8.0),
            gravity: self.gravity.clamp(0.0, 80.0),
            contact_skin: self.contact_skin.clamp(0.0, 0.50),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct FpsDemoRules {
    pub default_status: String,
    pub pickup_status: String,
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
            default_status: "Find blue cores, avoid hazards, reach extraction.".to_string(),
            pickup_status: "Core acquired.".to_string(),
            hazard_status: "You touched a hazard. Relaunch the demo to retry.".to_string(),
            goal_locked_status: "Beacon locked: collect all cores first.".to_string(),
            goal_complete_status: "Extraction complete. Stable runtime loop is playable.".to_string(),
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

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub struct FpsDemoState {
    pub title: String,
    pub objective: String,
    pub elapsed_sec: f32,
    pub pickups_collected: u32,
    pub pickups_total: u32,
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
        Self {
            title: title.into(),
            objective: objective.into(),
            elapsed_sec: 0.0,
            pickups_collected: 0,
            pickups_total,
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
            return format!("{} in {:.1}s", self.completed_progress_label, self.elapsed_sec.max(0.0));
        }
        if self.failed {
            return self.failed_progress_label.clone();
        }
        format!(
            "Cores {}/{} · {:.1}s",
            self.pickups_collected.min(self.pickups_total),
            self.pickups_total,
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
    PlayActivated,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GameReadyWorldLaunchGate {
    pub requested_frame: u64,
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
            GameReadyWorldLaunchGatePhase::Released | GameReadyWorldLaunchGatePhase::PlayActivated
        )
    }

    #[inline]
    pub const fn is_play_activated(&self) -> bool {
        matches!(self.phase, GameReadyWorldLaunchGatePhase::PlayActivated)
    }

    #[inline]
    pub fn release(&mut self, frame: u64, reason: impl Into<String>) {
        self.requested_frame = self.requested_frame.min(frame);
        self.released_frame = Some(frame);
        self.phase = GameReadyWorldLaunchGatePhase::Released;
        self.reason = reason.into();
        self.waiting_textures = 0;
    }

    #[inline]
    pub fn update_texture_counts(&mut self, waiting: u32, total: u32, failed: u32) {
        self.waiting_textures = waiting;
        self.total_textures = total;
        self.failed_textures = failed;
    }

    #[inline]
    pub fn mark_play_activated(&mut self) {
        self.phase = GameReadyWorldLaunchGatePhase::PlayActivated;
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum DisplayMode {
    #[default]
    Both,
    RuntimeHidden,
    GameOnly,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct DisplayVisibility {
    pub mode: DisplayMode,
}

impl DisplayVisibility {
    #[inline]
    pub const fn visible_in_authoring(self) -> bool {
        !matches!(self.mode, DisplayMode::GameOnly)
    }

    #[inline]
    pub const fn visible_in_game(self) -> bool {
        !matches!(self.mode, DisplayMode::RuntimeHidden)
    }
}
