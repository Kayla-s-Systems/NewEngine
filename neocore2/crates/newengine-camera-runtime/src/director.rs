#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_camera::{
    blend_resolved_camera_frames, CameraDirectorId, CameraDirectorMetadata, CameraDirectorOutput,
    CameraDirectorRunner, CameraDirectorState, CameraModeKind, CameraPostEffects,
    CameraRenderState, CameraResolvedFrame,
};

use crate::manager::CameraDirectorKind;

#[derive(Clone, Copy, Debug)]
pub struct CameraRuntimeDirectorOutput {
    pub kind: CameraDirectorKind,
    pub output: CameraDirectorOutput,
}

impl CameraRuntimeDirectorOutput {
    #[inline]
    pub const fn new(kind: CameraDirectorKind, output: CameraDirectorOutput) -> Self {
        Self { kind, output }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CameraRenderedDirector {
    pub kind: CameraDirectorKind,
    pub blend_level: f32,
    pub priority: i32,
    pub render_state: CameraRenderState,
    pub lock_input: bool,
}

impl CameraRenderedDirector {
    #[inline]
    fn from_output(frame: CameraRuntimeDirectorOutput) -> Self {
        Self {
            kind: frame.kind,
            blend_level: sanitize_blend(frame.output.blend_level),
            priority: frame.output.priority,
            render_state: frame.output.render_state,
            lock_input: frame.output.lock_input,
        }
    }
}

#[derive(Clone, Debug)]
pub struct CameraDirectorMixerOutput {
    pub frame: CameraResolvedFrame,
    pub dominant_director: CameraDirectorKind,
    pub dominant_blend_level: f32,
    pub rendered_directors: Vec<CameraRenderedDirector>,
    pub lock_input: bool,
}

#[derive(Clone, Debug, Default)]
pub struct CameraDirectorMixer {
    last_resolved: Option<CameraResolvedFrame>,
    last_dominant: Option<CameraDirectorKind>,
    rendered_directors: Vec<CameraRenderedDirector>,
}

impl CameraDirectorMixer {
    #[inline]
    pub fn last_resolved(&self) -> Option<CameraResolvedFrame> {
        self.last_resolved
    }

    #[inline]
    pub fn last_dominant(&self) -> Option<CameraDirectorKind> {
        self.last_dominant
    }

    #[inline]
    pub fn rendered_directors(&self) -> &[CameraRenderedDirector] {
        self.rendered_directors.as_slice()
    }

    pub fn resolve(
        &mut self,
        mut outputs: Vec<CameraRuntimeDirectorOutput>,
    ) -> Option<CameraDirectorMixerOutput> {
        outputs.retain(|candidate| {
            candidate.output.is_rendering() && sanitize_blend(candidate.output.blend_level) > 0.0
        });
        if outputs.is_empty() {
            self.rendered_directors.clear();
            return None;
        }

        // Blend lower-priority layers first and let higher-priority directors dominate on top.
        outputs.sort_by(|a, b| {
            a.output
                .priority
                .cmp(&b.output.priority)
                .then_with(|| director_sort_key(a.kind).cmp(&director_sort_key(b.kind)))
        });

        let mut resolved = outputs[0].output.frame;
        let mut rendered = Vec::with_capacity(outputs.len());
        let mut lock_input = false;

        for (index, candidate) in outputs.iter().copied().enumerate() {
            let blend_level = sanitize_blend(candidate.output.blend_level);
            let rendered_director = CameraRenderedDirector::from_output(candidate);
            lock_input |= rendered_director.lock_input;
            rendered.push(rendered_director);
            if index == 0 {
                continue;
            }
            resolved = blend_resolved_camera_frames(resolved, candidate.output.frame, blend_level);
        }

        let dominant = outputs
            .iter()
            .copied()
            .max_by(|a, b| {
                a.output
                    .priority
                    .cmp(&b.output.priority)
                    .then_with(|| {
                        sanitize_blend(a.output.blend_level)
                            .partial_cmp(&sanitize_blend(b.output.blend_level))
                            .unwrap_or(std::cmp::Ordering::Equal)
                    })
                    .then_with(|| director_sort_key(a.kind).cmp(&director_sort_key(b.kind)))
            })
            .unwrap_or(outputs[0]);

        let dominant_kind = dominant.kind;
        let dominant_blend_level = sanitize_blend(dominant.output.blend_level);
        self.last_resolved = Some(resolved);
        self.last_dominant = Some(dominant_kind);
        self.rendered_directors = rendered.clone();

        Some(CameraDirectorMixerOutput {
            frame: resolved,
            dominant_director: dominant_kind,
            dominant_blend_level,
            rendered_directors: rendered,
            lock_input,
        })
    }
}

#[inline]
fn sanitize_blend(value: f32) -> f32 {
    if value.is_finite() {
        value.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

#[inline]
fn director_sort_key(kind: CameraDirectorKind) -> u8 {
    match kind {
        CameraDirectorKind::Runtime => 10,
        CameraDirectorKind::Gameplay => 20,
        CameraDirectorKind::Cinematic => 30,
        CameraDirectorKind::Scripted => 40,
        CameraDirectorKind::Replay => 50,
        CameraDirectorKind::Cutscene => 60,
        CameraDirectorKind::Switch => 70,
        CameraDirectorKind::SyncedScene => 80,
        CameraDirectorKind::AnimScene => 90,
        CameraDirectorKind::Marketing => 100,
        CameraDirectorKind::Debug => 110,
    }
}

#[derive(Clone, Debug)]
pub struct StaticCameraDirectorRunner {
    state: CameraDirectorState,
    frame: Option<CameraResolvedFrame>,
    effects_override: Option<CameraPostEffects>,
}

impl StaticCameraDirectorRunner {
    #[inline]
    pub fn new(id: CameraDirectorId, metadata: CameraDirectorMetadata) -> Self {
        Self {
            state: CameraDirectorState::new(id, metadata),
            frame: None,
            effects_override: None,
        }
    }

    #[inline]
    pub fn with_frame(mut self, frame: CameraResolvedFrame) -> Self {
        self.frame = Some(frame);
        self
    }

    #[inline]
    pub fn set_frame(&mut self, frame: CameraResolvedFrame) {
        self.frame = Some(frame);
    }

    #[inline]
    pub fn set_effects_override(&mut self, effects: CameraPostEffects) {
        self.effects_override = Some(effects.sanitized());
    }

    #[inline]
    pub fn clear_effects_override(&mut self) {
        self.effects_override = None;
    }

    #[inline]
    pub fn render(&mut self, blend_in_sec: Option<f32>) {
        self.state.render(blend_in_sec);
    }

    #[inline]
    pub fn stop_rendering(&mut self, blend_out_sec: Option<f32>) {
        self.state.stop_rendering(blend_out_sec);
    }
}

impl CameraDirectorRunner for StaticCameraDirectorRunner {
    #[inline]
    fn state(&self) -> &CameraDirectorState {
        &self.state
    }

    #[inline]
    fn state_mut(&mut self) -> &mut CameraDirectorState {
        &mut self.state
    }

    #[inline]
    fn update_frame(&mut self, _dt: f32) -> Option<CameraResolvedFrame> {
        let mut frame = self.frame?;
        if let Some(effects) = self.effects_override {
            frame.effects = effects.sanitized();
        }
        Some(frame)
    }
}

#[derive(Clone, Debug)]
pub struct CinematicDirectorRunner {
    inner: StaticCameraDirectorRunner,
}

impl CinematicDirectorRunner {
    #[inline]
    pub fn new(id: u64, name: impl Into<String>) -> Self {
        Self {
            inner: StaticCameraDirectorRunner::new(
                CameraDirectorId(id),
                CameraDirectorMetadata::new(id, name, CameraModeKind::Cinematic),
            ),
        }
    }

    #[inline]
    pub fn set_frame(&mut self, frame: CameraResolvedFrame) {
        self.inner.set_frame(frame);
    }

    #[inline]
    pub fn render(&mut self, blend_in_sec: Option<f32>) {
        self.inner.render(blend_in_sec);
    }

    #[inline]
    pub fn stop_rendering(&mut self, blend_out_sec: Option<f32>) {
        self.inner.stop_rendering(blend_out_sec);
    }
}

impl CameraDirectorRunner for CinematicDirectorRunner {
    #[inline]
    fn state(&self) -> &CameraDirectorState {
        self.inner.state()
    }
    #[inline]
    fn state_mut(&mut self) -> &mut CameraDirectorState {
        self.inner.state_mut()
    }
    #[inline]
    fn update_frame(&mut self, dt: f32) -> Option<CameraResolvedFrame> {
        self.inner.update_frame(dt)
    }
}

#[derive(Clone, Debug)]
pub struct ScriptedDirectorRunner {
    inner: StaticCameraDirectorRunner,
}

impl ScriptedDirectorRunner {
    #[inline]
    pub fn new(id: u64, name: impl Into<String>) -> Self {
        Self {
            inner: StaticCameraDirectorRunner::new(
                CameraDirectorId(id),
                CameraDirectorMetadata::new(id, name, CameraModeKind::Scripted),
            ),
        }
    }

    #[inline]
    pub fn set_frame(&mut self, frame: CameraResolvedFrame) {
        self.inner.set_frame(frame);
    }
    #[inline]
    pub fn render(&mut self, blend_in_sec: Option<f32>) {
        self.inner.render(blend_in_sec);
    }
    #[inline]
    pub fn stop_rendering(&mut self, blend_out_sec: Option<f32>) {
        self.inner.stop_rendering(blend_out_sec);
    }
}

impl CameraDirectorRunner for ScriptedDirectorRunner {
    #[inline]
    fn state(&self) -> &CameraDirectorState {
        self.inner.state()
    }
    #[inline]
    fn state_mut(&mut self) -> &mut CameraDirectorState {
        self.inner.state_mut()
    }
    #[inline]
    fn update_frame(&mut self, dt: f32) -> Option<CameraResolvedFrame> {
        self.inner.update_frame(dt)
    }
}

#[derive(Clone, Debug)]
pub struct ReplayDirectorRunner {
    inner: StaticCameraDirectorRunner,
}

impl ReplayDirectorRunner {
    #[inline]
    pub fn new(id: u64, name: impl Into<String>) -> Self {
        Self {
            inner: StaticCameraDirectorRunner::new(
                CameraDirectorId(id),
                CameraDirectorMetadata::new(id, name, CameraModeKind::Replay),
            ),
        }
    }

    #[inline]
    pub fn set_frame(&mut self, frame: CameraResolvedFrame) {
        self.inner.set_frame(frame);
    }
    #[inline]
    pub fn render(&mut self, blend_in_sec: Option<f32>) {
        self.inner.render(blend_in_sec);
    }
    #[inline]
    pub fn stop_rendering(&mut self, blend_out_sec: Option<f32>) {
        self.inner.stop_rendering(blend_out_sec);
    }
}

impl CameraDirectorRunner for ReplayDirectorRunner {
    #[inline]
    fn state(&self) -> &CameraDirectorState {
        self.inner.state()
    }
    #[inline]
    fn state_mut(&mut self) -> &mut CameraDirectorState {
        self.inner.state_mut()
    }
    #[inline]
    fn update_frame(&mut self, dt: f32) -> Option<CameraResolvedFrame> {
        self.inner.update_frame(dt)
    }
}

pub type CutsceneDirectorRunner = StaticCameraDirectorRunner;
pub type SwitchDirectorRunner = StaticCameraDirectorRunner;
pub type SyncedSceneDirectorRunner = StaticCameraDirectorRunner;
pub type AnimSceneDirectorRunner = StaticCameraDirectorRunner;
pub type MarketingDirectorRunner = StaticCameraDirectorRunner;
pub type DebugDirectorRunner = StaticCameraDirectorRunner;
