use newengine_camera::{CameraPostEffects, RuntimeNavMode};

use crate::blend::{CameraFrameBlendCurve, CameraFrameBlendPlan};
use newengine_ecs::EntityId;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CameraDirectorKind {
    Runtime,
    Gameplay,
    Cinematic,
    Scripted,
    Replay,
    Cutscene,
    Switch,
    SyncedScene,
    AnimScene,
    Marketing,
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
    ScriptedPreview,
    ReplayPlayback,
    CutscenePlayback,
    SwitchBlend,
    SyncedScenePlayback,
    AnimScenePlayback,
    MarketingPreview,
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
    pub dominant_director: Option<CameraDirectorKind>,
    pub rendered_director_count: usize,
    pub director_lock_input: bool,
    pub viewport_changed: bool,
    pub pending_event_count: usize,
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
            dominant_director: None,
            rendered_director_count: 0,
            director_lock_input: false,
            viewport_changed: false,
            pending_event_count: 0,
        }
    }
}


#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CameraDirectorRuntimeSettings {
    pub blend_in_sec: f32,
    pub blend_out_sec: f32,
    pub blend_curve: CameraFrameBlendCurve,
    pub lock_input: bool,
    pub default_effects: CameraPostEffects,
}

impl Default for CameraDirectorRuntimeSettings {
    #[inline]
    fn default() -> Self {
        Self {
            blend_in_sec: 0.18,
            blend_out_sec: 0.14,
            blend_curve: CameraFrameBlendCurve::SmoothStep,
            lock_input: false,
            default_effects: CameraPostEffects::default(),
        }
    }
}

impl CameraDirectorRuntimeSettings {
    #[inline]
    pub const fn cut() -> Self {
        Self {
            blend_in_sec: 0.0,
            blend_out_sec: 0.0,
            blend_curve: CameraFrameBlendCurve::Linear,
            lock_input: false,
            default_effects: CameraPostEffects {
                dof: newengine_camera::CameraDepthOfFieldSettings {
                    near_start: 0.0,
                    near_end: 0.0,
                    far_start: 10_000.0,
                    far_end: 10_000.0,
                    blend_level: 0.0,
                },
                motion_blur: newengine_camera::CameraMotionBlurSettings {
                    strength: 0.0,
                    decay_rate: 0.5,
                },
                shake_amplitude: 0.0,
                exposure_bias: 0.0,
                jitter_px: newengine_math::Vec2::ZERO,
                high_quality_dof: false,
            },
        }
    }

    #[inline]
    pub fn sanitized(self) -> Self {
        Self {
            blend_in_sec: sanitize_duration(self.blend_in_sec),
            blend_out_sec: sanitize_duration(self.blend_out_sec),
            blend_curve: self.blend_curve,
            lock_input: self.lock_input,
            default_effects: self.default_effects.sanitized(),
        }
    }

    #[inline]
    pub fn blend_plan(self, entering: bool) -> CameraFrameBlendPlan {
        let this = self.sanitized();
        let duration = if entering { this.blend_in_sec } else { this.blend_out_sec };
        if duration <= 0.0 {
            CameraFrameBlendPlan::cut()
        } else {
            CameraFrameBlendPlan::timed(duration, this.blend_curve).with_lock_input(this.lock_input)
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CameraRuntimeSettings {
    pub runtime: CameraDirectorRuntimeSettings,
    pub gameplay: CameraDirectorRuntimeSettings,
    pub cinematic: CameraDirectorRuntimeSettings,
    pub scripted: CameraDirectorRuntimeSettings,
    pub replay: CameraDirectorRuntimeSettings,
    pub cutscene: CameraDirectorRuntimeSettings,
    pub switch: CameraDirectorRuntimeSettings,
    pub synced_scene: CameraDirectorRuntimeSettings,
    pub anim_scene: CameraDirectorRuntimeSettings,
    pub marketing: CameraDirectorRuntimeSettings,
    pub debug: CameraDirectorRuntimeSettings,
}

impl Default for CameraRuntimeSettings {
    #[inline]
    fn default() -> Self {
        Self {
            runtime: CameraDirectorRuntimeSettings {
                blend_in_sec: 0.10,
                blend_out_sec: 0.10,
                ..CameraDirectorRuntimeSettings::default()
            },
            gameplay: CameraDirectorRuntimeSettings {
                blend_in_sec: 0.16,
                blend_out_sec: 0.14,
                ..CameraDirectorRuntimeSettings::default()
            },
            cinematic: CameraDirectorRuntimeSettings {
                blend_in_sec: 0.35,
                blend_out_sec: 0.24,
                lock_input: true,
                default_effects: CameraPostEffects {
                    motion_blur: newengine_camera::CameraMotionBlurSettings {
                        strength: 0.08,
                        decay_rate: 0.5,
                    },
                    ..CameraPostEffects::default()
                },
                ..CameraDirectorRuntimeSettings::default()
            },
            scripted: CameraDirectorRuntimeSettings {
                blend_in_sec: 0.25,
                blend_out_sec: 0.20,
                lock_input: true,
                ..CameraDirectorRuntimeSettings::default()
            },
            replay: CameraDirectorRuntimeSettings {
                blend_in_sec: 0.20,
                blend_out_sec: 0.20,
                ..CameraDirectorRuntimeSettings::default()
            },
            cutscene: CameraDirectorRuntimeSettings {
                blend_in_sec: 0.0,
                blend_out_sec: 0.0,
                lock_input: true,
                ..CameraDirectorRuntimeSettings::default()
            },
            switch: CameraDirectorRuntimeSettings {
                blend_in_sec: 0.18,
                blend_out_sec: 0.18,
                lock_input: true,
                ..CameraDirectorRuntimeSettings::default()
            },
            synced_scene: CameraDirectorRuntimeSettings {
                blend_in_sec: 0.12,
                blend_out_sec: 0.12,
                lock_input: true,
                ..CameraDirectorRuntimeSettings::default()
            },
            anim_scene: CameraDirectorRuntimeSettings {
                blend_in_sec: 0.12,
                blend_out_sec: 0.12,
                lock_input: true,
                ..CameraDirectorRuntimeSettings::default()
            },
            marketing: CameraDirectorRuntimeSettings {
                blend_in_sec: 0.24,
                blend_out_sec: 0.24,
                ..CameraDirectorRuntimeSettings::default()
            },
            debug: CameraDirectorRuntimeSettings::cut(),
        }
    }
}

impl CameraRuntimeSettings {
    #[inline]
    pub fn for_director(&self, director: CameraDirectorKind) -> CameraDirectorRuntimeSettings {
        match director {
            CameraDirectorKind::Runtime => self.runtime,
            CameraDirectorKind::Gameplay => self.gameplay,
            CameraDirectorKind::Cinematic => self.cinematic,
            CameraDirectorKind::Scripted => self.scripted,
            CameraDirectorKind::Replay => self.replay,
            CameraDirectorKind::Cutscene => self.cutscene,
            CameraDirectorKind::Switch => self.switch,
            CameraDirectorKind::SyncedScene => self.synced_scene,
            CameraDirectorKind::AnimScene => self.anim_scene,
            CameraDirectorKind::Marketing => self.marketing,
            CameraDirectorKind::Debug => self.debug,
        }
    }

    #[inline]
    pub fn set_for_director(
        &mut self,
        director: CameraDirectorKind,
        settings: CameraDirectorRuntimeSettings,
    ) {
        let settings = settings.sanitized();
        match director {
            CameraDirectorKind::Runtime => self.runtime = settings,
            CameraDirectorKind::Gameplay => self.gameplay = settings,
            CameraDirectorKind::Cinematic => self.cinematic = settings,
            CameraDirectorKind::Scripted => self.scripted = settings,
            CameraDirectorKind::Replay => self.replay = settings,
            CameraDirectorKind::Cutscene => self.cutscene = settings,
            CameraDirectorKind::Switch => self.switch = settings,
            CameraDirectorKind::SyncedScene => self.synced_scene = settings,
            CameraDirectorKind::AnimScene => self.anim_scene = settings,
            CameraDirectorKind::Marketing => self.marketing = settings,
            CameraDirectorKind::Debug => self.debug = settings,
        }
    }
}

#[inline]
fn sanitize_duration(value: f32) -> f32 {
    if value.is_finite() && value > 0.0 { value } else { 0.0 }
}
