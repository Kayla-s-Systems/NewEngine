use newengine_camera::{CameraFrame, EditorNavMode};
use newengine_core::host_events::CursorState;

use crate::blend::{CameraFrameBlendCurve, CameraFrameBlendPlan, CameraFrameBlendState};

use super::types::{
    CameraDirectorKind, CameraDirectorRequest, CameraInputContext, CameraRuntimeMode,
    CameraRuntimeReport, CameraRuntimeWorldState, CameraTransitionPhase, CameraTransitionPlan,
    CameraTransitionState,
};

#[derive(Clone, Debug)]
pub struct CameraManagerResource {
    pub active_director: CameraDirectorKind,
    pub active_mode: CameraRuntimeMode,
    pub input_context: CameraInputContext,
    pub target_entity: Option<newengine_ecs::EntityId>,
    pub transition: CameraTransitionState,
    pub gate_blocked: bool,
    pub last_cursor: CursorState,
    pub frame_blend: CameraFrameBlendState,
    pending_request: Option<CameraDirectorRequest>,
}

impl Default for CameraManagerResource {
    #[inline]
    fn default() -> Self {
        Self {
            active_director: CameraDirectorKind::Editor,
            active_mode: CameraRuntimeMode::EditorOrbit,
            input_context: CameraInputContext::EditorNav,
            target_entity: None,
            transition: CameraTransitionState::default(),
            gate_blocked: false,
            last_cursor: CursorState::released(),
            frame_blend: CameraFrameBlendState::default(),
            pending_request: None,
        }
    }
}

impl CameraManagerResource {
    #[inline]
    pub fn advance(&mut self, dt: f32) {
        if !(dt.is_finite() && dt > 0.0) {
            return;
        }
        match self.transition.phase {
            CameraTransitionPhase::Idle => {}
            CameraTransitionPhase::Pending => {
                self.transition.phase = CameraTransitionPhase::Blending;
                self.transition.elapsed_sec = 0.0;
            }
            CameraTransitionPhase::Blending => {
                self.transition.elapsed_sec += dt;
                let done = self
                    .transition
                    .plan
                    .map(|plan| self.transition.elapsed_sec >= plan.duration_sec.max(0.0))
                    .unwrap_or(true);
                if done {
                    self.transition = CameraTransitionState::default();
                }
            }
        }
    }

    #[inline]
    pub fn sync_world_state(&mut self, state: CameraRuntimeWorldState) {
        let (desired_director, desired_mode, desired_context, desired_target) =
            desired_camera_policy(state);

        let changed = self.active_director != desired_director
            || self.active_mode != desired_mode
            || self.input_context != desired_context
            || self.gate_blocked != state.gate_blocked;

        if changed {
            self.begin_transition(CameraTransitionPlan {
                from_director: self.active_director,
                to_director: desired_director,
                from_mode: self.active_mode,
                to_mode: desired_mode,
                duration_sec: transition_duration(self.active_director, desired_director),
                lock_input: matches!(
                    desired_context,
                    CameraInputContext::TransitionLocked | CameraInputContext::CinematicLocked
                ),
                preserve_pose: true,
                frame_blend: CameraFrameBlendPlan::timed(
                    transition_duration(self.active_director, desired_director),
                    CameraFrameBlendCurve::SmoothStep,
                )
                .with_lock_input(matches!(
                    desired_context,
                    CameraInputContext::TransitionLocked | CameraInputContext::CinematicLocked
                )),
            });

            log::info!(
                "camera runtime: director={:?} mode={:?} context={:?} gate_blocked={}",
                desired_director,
                desired_mode,
                desired_context,
                state.gate_blocked
            );
        }

        if self.target_entity != desired_target {
            self.pending_request = match (self.target_entity, desired_target) {
                (Some(_), None) => Some(CameraDirectorRequest::ReleasePlayer),
                (_, Some(player)) => Some(CameraDirectorRequest::PossessPlayer { player }),
                (None, None) => None,
            };
        }

        self.active_director = desired_director;
        self.active_mode = desired_mode;
        self.input_context = desired_context;
        self.gate_blocked = state.gate_blocked;
        self.target_entity = desired_target;
    }

    #[inline]
    pub fn sync_editor_mode_from_controller(&mut self, mode: EditorNavMode) {
        if self.active_director != CameraDirectorKind::Editor {
            return;
        }
        self.active_mode = match mode {
            EditorNavMode::Orbit => CameraRuntimeMode::EditorOrbit,
            EditorNavMode::Fly => CameraRuntimeMode::EditorFly,
        };
    }

    #[inline]
    pub fn take_pending_request(&mut self) -> Option<CameraDirectorRequest> {
        self.pending_request.take()
    }

    #[inline]
    pub fn wants_navigation_input(&self) -> bool {
        matches!(self.input_context, CameraInputContext::EditorNav)
    }

    #[inline]
    pub fn set_last_cursor(&mut self, cursor: CursorState) {
        self.last_cursor = cursor;
    }


    #[inline]
    pub fn resolve_camera_frame(&mut self, frame: CameraFrame, dt: f32) -> CameraFrame {
        self.frame_blend.resolve(frame, dt)
    }

    #[inline]
    pub fn report(&self) -> CameraRuntimeReport {
        CameraRuntimeReport {
            active_director: self.active_director,
            active_mode: self.active_mode,
            target_entity: self.target_entity,
            transition: self.transition,
            input_context: self.input_context,
            gate_blocked: self.gate_blocked,
            frame_blend_active: self.frame_blend.is_active(),
            frame_blend_alpha: self.frame_blend.alpha(),
        }
    }

    #[inline]
    fn begin_transition(&mut self, plan: CameraTransitionPlan) {
        if plan.duration_sec <= 0.0 {
            self.transition = CameraTransitionState::default();
            self.frame_blend.begin(CameraFrameBlendPlan::cut());
            return;
        }
        self.frame_blend.begin(plan.frame_blend);
        self.transition = CameraTransitionState {
            phase: CameraTransitionPhase::Pending,
            plan: Some(plan),
            elapsed_sec: 0.0,
        };
    }
}

#[inline]
fn desired_camera_policy(
    state: CameraRuntimeWorldState,
) -> (
    CameraDirectorKind,
    CameraRuntimeMode,
    CameraInputContext,
    Option<newengine_ecs::EntityId>,
) {
    if state.runtime_requested && state.gate_blocked {
        return (
            CameraDirectorKind::Gameplay,
            CameraRuntimeMode::GameplayPreview,
            CameraInputContext::TransitionLocked,
            None,
        );
    }

    if state.wants_direct_player_control {
        return (
            CameraDirectorKind::Gameplay,
            CameraRuntimeMode::GameplayFirstPerson,
            CameraInputContext::GameplayLook,
            state.player,
        );
    }

    if state.public_runtime_active {
        return (
            CameraDirectorKind::Gameplay,
            CameraRuntimeMode::GameplayPreview,
            CameraInputContext::None,
            None,
        );
    }

    let editor_mode = match state.editor_nav_mode {
        EditorNavMode::Orbit => CameraRuntimeMode::EditorOrbit,
        EditorNavMode::Fly => CameraRuntimeMode::EditorFly,
    };
    (
        CameraDirectorKind::Editor,
        editor_mode,
        CameraInputContext::EditorNav,
        None,
    )
}

#[inline]
fn transition_duration(from: CameraDirectorKind, to: CameraDirectorKind) -> f32 {
    if from == to {
        0.10
    } else {
        0.18
    }
}
