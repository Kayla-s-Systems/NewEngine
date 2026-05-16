use newengine_camera::RuntimeNavMode;

use crate::blend::CameraFrameBlendPlan;
use newengine_ecs::EntityId;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CameraDirectorKind {
    Runtime,
    Gameplay,
    Cinematic,
    Scripted,
    Replay,
    Debug,
}

impl Default for CameraDirectorKind {
    #[inline]
    fn default() -> Self {
        Self::Runtime
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CameraRuntimeMode {
    RuntimeOrbit,
    RuntimeFly,
    GameplayPreview,
    GameplayFirstPerson,
    GameplayThirdPersonFollow,
    GameplayThirdPersonAim,
    CinematicPreview,
    DebugFree,
}

impl Default for CameraRuntimeMode {
    #[inline]
    fn default() -> Self {
        Self::RuntimeOrbit
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CameraInputContext {
    RuntimeNav,
    GameplayLook,
    TransitionLocked,
    CinematicLocked,
    None,
}

impl Default for CameraInputContext {
    #[inline]
    fn default() -> Self {
        Self::RuntimeNav
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CameraTransitionPhase {
    Idle,
    Pending,
    Blending,
}

impl Default for CameraTransitionPhase {
    #[inline]
    fn default() -> Self {
        Self::Idle
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CameraTransitionPlan {
    pub from_director: CameraDirectorKind,
    pub to_director: CameraDirectorKind,
    pub from_mode: CameraRuntimeMode,
    pub to_mode: CameraRuntimeMode,
    pub duration_sec: f32,
    pub lock_input: bool,
    pub preserve_pose: bool,
    pub frame_blend: CameraFrameBlendPlan,
}

impl Default for CameraTransitionPlan {
    #[inline]
    fn default() -> Self {
        Self {
            from_director: CameraDirectorKind::Runtime,
            to_director: CameraDirectorKind::Runtime,
            from_mode: CameraRuntimeMode::RuntimeOrbit,
            to_mode: CameraRuntimeMode::RuntimeOrbit,
            duration_sec: 0.0,
            lock_input: false,
            preserve_pose: true,
            frame_blend: CameraFrameBlendPlan::cut(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CameraTransitionState {
    pub phase: CameraTransitionPhase,
    pub plan: Option<CameraTransitionPlan>,
    pub elapsed_sec: f32,
}

impl Default for CameraTransitionState {
    #[inline]
    fn default() -> Self {
        Self {
            phase: CameraTransitionPhase::Idle,
            plan: None,
            elapsed_sec: 0.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CameraDirectorRequest {
    PossessPlayer { player: EntityId },
    ReleasePlayer,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CameraRuntimeWorldState {
    pub game_nav_mode: RuntimeNavMode,
    pub runtime_requested: bool,
    pub public_runtime_active: bool,
    pub wants_direct_player_control: bool,
    pub gate_blocked: bool,
    pub player: Option<EntityId>,
}

impl Default for CameraRuntimeWorldState {
    #[inline]
    fn default() -> Self {
        Self {
            game_nav_mode: RuntimeNavMode::Orbit,
            runtime_requested: false,
            public_runtime_active: false,
            wants_direct_player_control: false,
            gate_blocked: false,
            player: None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CameraRuntimeReport {
    pub active_director: CameraDirectorKind,
    pub active_mode: CameraRuntimeMode,
    pub target_entity: Option<EntityId>,
    pub transition: CameraTransitionState,
    pub input_context: CameraInputContext,
    pub gate_blocked: bool,
    pub frame_blend_active: bool,
    pub frame_blend_alpha: f32,
}

impl Default for CameraRuntimeReport {
    #[inline]
    fn default() -> Self {
        Self {
            active_director: CameraDirectorKind::Runtime,
            active_mode: CameraRuntimeMode::RuntimeOrbit,
            target_entity: None,
            transition: CameraTransitionState::default(),
            input_context: CameraInputContext::RuntimeNav,
            gate_blocked: false,
            frame_blend_active: false,
            frame_blend_alpha: 1.0,
        }
    }
}
