use newengine_camera::{
    CameraDirectorId, CameraDirectorOutput, CameraFrame, CameraPostEffects, CameraRenderState,
    CameraResolvedFrame, RuntimeNavMode,
};
use newengine_camera_api::CameraViewMode;
use newengine_core::host_events::CursorState;

use crate::blend::{CameraFrameBlendCurve, CameraFrameBlendPlan, CameraFrameBlendState};
use crate::director::{CameraDirectorMixer, CameraRuntimeDirectorOutput};
use crate::events::{CameraRuntimeEvent, CameraRuntimeEventKind};
use crate::viewport::CameraViewportManagerResource;

use super::types::{
    CameraDirectorKind, CameraDirectorRequest, CameraDirectorRuntimeSettings, CameraInputContext,
    CameraRuntimeMode, CameraRuntimeReport, CameraRuntimeSettings, CameraRuntimeWorldState,
    CameraTransitionPhase, CameraTransitionPlan, CameraTransitionState,
};

const CAMERA_EVENT_QUEUE_LIMIT: usize = 64;

#[derive(Clone, Debug)]
pub struct CameraManagerResource {
    pub active_director: CameraDirectorKind,
    pub active_mode: CameraRuntimeMode,
    pub view_mode: CameraViewMode,
    pub input_context: CameraInputContext,
    pub target_entity: Option<newengine_ecs::EntityId>,
    pub transition: CameraTransitionState,
    pub gate_blocked: bool,
    pub last_cursor: CursorState,
    pub frame_blend: CameraFrameBlendState,
    pub settings: CameraRuntimeSettings,
    pub director_mixer: CameraDirectorMixer,
    pub viewport_manager: CameraViewportManagerResource,
    pending_request: Option<CameraDirectorRequest>,
    pending_director_outputs: Vec<CameraRuntimeDirectorOutput>,
    pending_events: Vec<CameraRuntimeEvent>,
    last_resolved_frame: Option<CameraResolvedFrame>,
    last_dominant_director: Option<CameraDirectorKind>,
    director_lock_input: bool,
}

impl Default for CameraManagerResource {
    #[inline]
    fn default() -> Self {
        Self {
            active_director: CameraDirectorKind::Runtime,
            active_mode: CameraRuntimeMode::RuntimeOrbit,
            view_mode: CameraViewMode::FirstPerson,
            input_context: CameraInputContext::RuntimeNav,
            target_entity: None,
            transition: CameraTransitionState::default(),
            gate_blocked: false,
            last_cursor: CursorState::released(),
            frame_blend: CameraFrameBlendState::default(),
            settings: CameraRuntimeSettings::default(),
            director_mixer: CameraDirectorMixer::default(),
            viewport_manager: CameraViewportManagerResource::default(),
            pending_request: None,
            pending_director_outputs: Vec::new(),
            pending_events: Vec::new(),
            last_resolved_frame: None,
            last_dominant_director: None,
            director_lock_input: false,
        }
    }
}

impl CameraManagerResource {
    #[inline]
    pub fn advance(&mut self, dt: f32) {
        self.viewport_manager.update(dt);
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
                    if let Some(plan) = self.transition.plan {
                        self.push_event(
                            CameraRuntimeEvent::new(
                                CameraRuntimeEventKind::TransitionCompleted,
                                plan.to_director,
                            )
                            .with_previous(Some(plan.from_director))
                            .with_elapsed(self.transition.elapsed_sec),
                        );
                    }
                    self.transition = CameraTransitionState::default();
                }
            }
        }
    }

    #[inline]
    pub fn sync_world_state(&mut self, state: CameraRuntimeWorldState) {
        let (desired_director, desired_mode, desired_context, desired_target) =
            desired_camera_policy(state);

        let view_changed = self.view_mode != state.view_mode;
        let changed = self.active_director != desired_director
            || self.active_mode != desired_mode
            || view_changed
            || self.input_context != desired_context
            || self.gate_blocked != state.gate_blocked;

        if changed {
            let duration = transition_duration_from_settings(
                &self.settings,
                self.active_director,
                desired_director,
            );
            self.begin_transition(CameraTransitionPlan {
                from_director: self.active_director,
                to_director: desired_director,
                from_mode: self.active_mode,
                to_mode: desired_mode,
                duration_sec: duration,
                lock_input: matches!(
                    desired_context,
                    CameraInputContext::TransitionLocked | CameraInputContext::CinematicLocked
                ) || self.settings.for_director(desired_director).lock_input,
                preserve_pose: true,
                frame_blend: CameraFrameBlendPlan::timed(
                    duration,
                    self.settings.for_director(desired_director).blend_curve,
                )
                .with_lock_input(
                    matches!(
                        desired_context,
                        CameraInputContext::TransitionLocked | CameraInputContext::CinematicLocked
                    ) || self.settings.for_director(desired_director).lock_input,
                ),
            });

            self.push_event(
                CameraRuntimeEvent::new(
                    CameraRuntimeEventKind::DirectorRequested,
                    desired_director,
                )
                .with_previous(Some(self.active_director)),
            );

            newengine_ulog_api::ulog::info!(
                "camera runtime: director={:?} mode={:?} view={:?} context={:?} gate_blocked={}",
                desired_director,
                desired_mode,
                state.view_mode,
                desired_context,
                state.gate_blocked
            );
        }

        if self.target_entity != desired_target || view_changed {
            self.pending_request = match (self.target_entity, desired_target) {
                (Some(_), None) => Some(CameraDirectorRequest::ReleasePlayer),
                (_, Some(player)) => Some(CameraDirectorRequest::PossessPlayer { player }),
                (None, None) => None,
            };
        }

        self.active_director = desired_director;
        self.active_mode = desired_mode;
        self.view_mode = state.view_mode;
        self.input_context = desired_context;
        self.gate_blocked = state.gate_blocked;
        self.target_entity = desired_target;
    }

    #[inline]
    pub fn sync_runtime_nav_mode_from_controller(&mut self, mode: RuntimeNavMode) {
        if self.active_director != CameraDirectorKind::Runtime {
            return;
        }
        self.active_mode = match mode {
            RuntimeNavMode::Orbit => CameraRuntimeMode::RuntimeOrbit,
            RuntimeNavMode::Fly => CameraRuntimeMode::RuntimeFly,
        };
    }

    #[inline]
    pub fn set_view_mode(&mut self, mode: CameraViewMode) {
        self.view_mode = mode;
    }

    #[inline]
    pub fn active_view_mode(&self) -> CameraViewMode {
        self.view_mode
    }

    #[inline]
    pub fn take_pending_request(&mut self) -> Option<CameraDirectorRequest> {
        self.pending_request.take()
    }

    #[inline]
    pub fn wants_navigation_input(&self) -> bool {
        matches!(self.input_context, CameraInputContext::RuntimeNav) && !self.director_lock_input
    }

    #[inline]
    pub fn set_last_cursor(&mut self, cursor: CursorState) {
        self.last_cursor = cursor;
    }

    #[inline]
    pub fn set_director_settings(
        &mut self,
        director: CameraDirectorKind,
        settings: CameraDirectorRuntimeSettings,
    ) {
        self.settings.set_for_director(director, settings);
        self.push_event(CameraRuntimeEvent::new(
            CameraRuntimeEventKind::EffectsChanged,
            director,
        ));
    }

    #[inline]
    pub fn set_director_effects(
        &mut self,
        director: CameraDirectorKind,
        effects: CameraPostEffects,
    ) {
        let mut settings = self.settings.for_director(director);
        settings.default_effects = effects.sanitized();
        self.set_director_settings(director, settings);
    }

    #[inline]
    pub fn submit_director_output(
        &mut self,
        kind: CameraDirectorKind,
        output: CameraDirectorOutput,
    ) {
        self.pending_director_outputs
            .push(CameraRuntimeDirectorOutput::new(kind, output));
    }

    #[inline]
    pub fn last_resolved_frame(&self) -> Option<CameraResolvedFrame> {
        self.last_resolved_frame
    }

    #[inline]
    pub fn last_post_effects(&self) -> Option<CameraPostEffects> {
        self.last_resolved_frame.map(|frame| frame.effects)
    }

    pub fn take_events(&mut self) -> Vec<CameraRuntimeEvent> {
        let mut out = Vec::new();
        std::mem::swap(&mut out, &mut self.pending_events);
        out
    }

    #[inline]
    pub fn pending_event_count(&self) -> usize {
        self.pending_events.len()
    }

    #[inline]
    pub fn resolve_camera_frame(&mut self, frame: CameraFrame, dt: f32) -> CameraFrame {
        let active_output = self.active_director_output(frame);
        let mut outputs = Vec::with_capacity(self.pending_director_outputs.len() + 1);
        outputs.push(active_output);
        outputs.append(&mut self.pending_director_outputs);

        let mixed = self.director_mixer.resolve(outputs);
        let Some(mixed) = mixed else {
            self.director_lock_input = false;
            self.last_resolved_frame = Some(CameraResolvedFrame::new(frame));
            return frame;
        };

        self.director_lock_input = mixed.lock_input;
        if self.last_dominant_director != Some(mixed.dominant_director) {
            self.push_event(
                CameraRuntimeEvent::new(
                    CameraRuntimeEventKind::DominantDirectorChanged,
                    mixed.dominant_director,
                )
                .with_previous(self.last_dominant_director)
                .with_blend(mixed.dominant_blend_level),
            );
            self.last_dominant_director = Some(mixed.dominant_director);
        }

        let blended_frame = self.frame_blend.resolve(mixed.frame.frame, dt);
        let final_frame = apply_effect_jitter(blended_frame, mixed.frame.effects);
        let final_frame = self.viewport_manager.present_frame(final_frame, dt);
        if self.viewport_manager.changed_this_update() {
            self.push_event(
                CameraRuntimeEvent::new(
                    CameraRuntimeEventKind::ViewportChanged,
                    mixed.dominant_director,
                )
                .with_blend(mixed.dominant_blend_level),
            );
        }
        let resolved = CameraResolvedFrame::with_effects(final_frame, mixed.frame.effects);
        self.last_resolved_frame = Some(resolved);
        final_frame
    }

    #[inline]
    pub fn report(&self) -> CameraRuntimeReport {
        CameraRuntimeReport {
            active_director: self.active_director,
            active_mode: self.active_mode,
            view_mode: self.view_mode,
            target_entity: self.target_entity,
            transition: self.transition,
            input_context: self.input_context,
            gate_blocked: self.gate_blocked,
            frame_blend_active: self.frame_blend.is_active(),
            frame_blend_alpha: self.frame_blend.alpha(),
            dominant_director: self.director_mixer.last_dominant(),
            rendered_director_count: self.director_mixer.rendered_directors().len(),
            director_lock_input: self.director_lock_input,
            viewport_changed: self.viewport_manager.changed_this_update(),
            pending_event_count: self.pending_events.len(),
        }
    }

    #[inline]
    fn begin_transition(&mut self, plan: CameraTransitionPlan) {
        self.push_event(
            CameraRuntimeEvent::new(CameraRuntimeEventKind::TransitionStarted, plan.to_director)
                .with_previous(Some(plan.from_director)),
        );
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

    fn active_director_output(&self, frame: CameraFrame) -> CameraRuntimeDirectorOutput {
        let settings = self.settings.for_director(self.active_director).sanitized();
        let effects = effects_for_mode(self.active_mode, settings.default_effects).sanitized();
        let resolved = CameraResolvedFrame::with_effects(frame, effects);
        let render_state = match self.transition.phase {
            CameraTransitionPhase::Idle => CameraRenderState::FullyRendering,
            CameraTransitionPhase::Pending | CameraTransitionPhase::Blending => {
                CameraRenderState::InterpolatingIn
            }
        };
        let blend_level = match self.transition.phase {
            CameraTransitionPhase::Idle | CameraTransitionPhase::Pending => 1.0,
            CameraTransitionPhase::Blending => self
                .transition
                .plan
                .map(|plan| {
                    let duration = plan.duration_sec.max(1.0e-6);
                    CameraFrameBlendPlan::timed(duration, CameraFrameBlendCurve::SmoothStep)
                        .sample(self.transition.elapsed_sec)
                })
                .unwrap_or(1.0),
        };
        let output = CameraDirectorOutput {
            id: CameraDirectorId(director_id(self.active_director)),
            frame: resolved,
            render_state,
            priority: priority_for_director(self.active_director),
            blend_level,
            lock_input: settings.lock_input
                || self
                    .transition
                    .plan
                    .map(|plan| plan.lock_input)
                    .unwrap_or(false),
        };
        CameraRuntimeDirectorOutput::new(self.active_director, output)
    }

    fn push_event(&mut self, event: CameraRuntimeEvent) {
        if self.pending_events.len() >= CAMERA_EVENT_QUEUE_LIMIT {
            let overflow = self.pending_events.len() + 1 - CAMERA_EVENT_QUEUE_LIMIT;
            self.pending_events.drain(0..overflow);
        }
        self.pending_events.push(event);
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
        let mode = match state.view_mode {
            CameraViewMode::FirstPerson => CameraRuntimeMode::GameplayFirstPerson,
            CameraViewMode::ThirdPersonFollow => CameraRuntimeMode::GameplayThirdPersonFollow,
            CameraViewMode::ThirdPersonAim => CameraRuntimeMode::GameplayThirdPersonAim,
        };
        return (
            CameraDirectorKind::Gameplay,
            mode,
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

    let runtime_nav_mode = match state.game_nav_mode {
        RuntimeNavMode::Orbit => CameraRuntimeMode::RuntimeOrbit,
        RuntimeNavMode::Fly => CameraRuntimeMode::RuntimeFly,
    };
    (
        CameraDirectorKind::Runtime,
        runtime_nav_mode,
        CameraInputContext::RuntimeNav,
        None,
    )
}

#[inline]
fn transition_duration_from_settings(
    settings: &CameraRuntimeSettings,
    from: CameraDirectorKind,
    to: CameraDirectorKind,
) -> f32 {
    if from == to {
        return 0.10;
    }
    let out = settings.for_director(from).blend_out_sec;
    let input = settings.for_director(to).blend_in_sec;
    out.max(input).max(0.0)
}

#[inline]
fn effects_for_mode(mode: CameraRuntimeMode, base: CameraPostEffects) -> CameraPostEffects {
    let mut effects = base.sanitized();
    match mode {
        CameraRuntimeMode::GameplayFirstPerson | CameraRuntimeMode::GameplayThirdPersonAim => {
            effects.motion_blur.strength = effects.motion_blur.strength.max(0.03);
        }
        CameraRuntimeMode::CinematicPreview
        | CameraRuntimeMode::ScriptedPreview
        | CameraRuntimeMode::CutscenePlayback
        | CameraRuntimeMode::MarketingPreview => {
            effects.motion_blur.strength = effects.motion_blur.strength.max(0.08);
            effects.dof.blend_level = effects.dof.blend_level.max(0.25);
        }
        CameraRuntimeMode::ReplayPlayback => {
            effects.motion_blur.strength = effects.motion_blur.strength.max(0.05);
        }
        CameraRuntimeMode::DebugFree
        | CameraRuntimeMode::RuntimeOrbit
        | CameraRuntimeMode::RuntimeFly
        | CameraRuntimeMode::GameplayPreview
        | CameraRuntimeMode::GameplayThirdPersonFollow
        | CameraRuntimeMode::SwitchBlend
        | CameraRuntimeMode::SyncedScenePlayback
        | CameraRuntimeMode::AnimScenePlayback => {}
    }
    effects.sanitized()
}

#[inline]
fn apply_effect_jitter(frame: CameraFrame, effects: CameraPostEffects) -> CameraFrame {
    let jitter = effects.jitter_px;
    if jitter.length_squared() <= 1.0e-10 {
        return frame;
    }
    CameraFrame::build(
        frame.channel,
        frame.rig,
        frame.projection,
        frame.viewport,
        frame.jitter_px + jitter,
    )
}

#[inline]
fn priority_for_director(kind: CameraDirectorKind) -> i32 {
    match kind {
        CameraDirectorKind::Debug => 1000,
        CameraDirectorKind::Cutscene => 950,
        CameraDirectorKind::Cinematic => 900,
        CameraDirectorKind::Switch => 880,
        CameraDirectorKind::Scripted => 850,
        CameraDirectorKind::SyncedScene => 830,
        CameraDirectorKind::AnimScene => 820,
        CameraDirectorKind::Replay => 760,
        CameraDirectorKind::Marketing => 700,
        CameraDirectorKind::Gameplay => 500,
        CameraDirectorKind::Runtime => 300,
    }
}

#[inline]
fn director_id(kind: CameraDirectorKind) -> u64 {
    match kind {
        CameraDirectorKind::Runtime => 1,
        CameraDirectorKind::Gameplay => 2,
        CameraDirectorKind::Cinematic => 3,
        CameraDirectorKind::Scripted => 4,
        CameraDirectorKind::Replay => 5,
        CameraDirectorKind::Cutscene => 6,
        CameraDirectorKind::Switch => 7,
        CameraDirectorKind::SyncedScene => 8,
        CameraDirectorKind::AnimScene => 9,
        CameraDirectorKind::Marketing => 10,
        CameraDirectorKind::Debug => 11,
    }
}
