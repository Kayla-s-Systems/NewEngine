#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_system_contracts::{
    ScreenOverlayProgress, ScreenOverlaySubsystem, ScreenOverlaySubsystemId,
    ScreenOverlaySubsystemPhase,
};

pub(crate) const START_ENGINE_BOOTSTRAP_BASE_PROGRESS: f32 = 0.74;
pub(crate) const START_ENGINE_BOOTSTRAP_SPAN_PROGRESS: f32 = 0.18;
pub(crate) const OVERLAY_LOG_PROGRESS_EPSILON: f32 = 0.01;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RuntimeBootstrapStage {
    AwaitingWindow,
    AnnounceLoadEnginePlugins,
    LoadEnginePlugins,
    AnnounceStartEngine,
    StartEngine,
    AnnounceEnterRuntime,
    EmitWindowReady,
    ReadyOverlay,
    Running,
}

#[derive(Debug, Clone)]
pub(crate) struct RuntimeBootstrapOverlayState {
    pub(crate) title: String,
    pub(crate) status: String,
    pub(crate) detail: String,
    pub(crate) progress_01: f32,
}

impl Default for RuntimeBootstrapOverlayState {
    #[inline]
    fn default() -> Self {
        Self {
            title: "NORTH STAR ENGINE".to_owned(),
            status: "Waiting for platform window...".to_owned(),
            detail: "The runtime shell is preparing the first visible frame.".to_owned(),
            progress_01: 0.0,
        }
    }
}

#[inline]
pub(crate) fn map_engine_startup_progress_to_bootstrap(engine_progress_01: f32) -> f32 {
    START_ENGINE_BOOTSTRAP_BASE_PROGRESS
        + engine_progress_01.clamp(0.0, 1.0) * START_ENGINE_BOOTSTRAP_SPAN_PROGRESS
}

pub(crate) fn subsystem_wait(
    id: ScreenOverlaySubsystemId,
    state_label: impl Into<String>,
    detail: impl Into<String>,
) -> ScreenOverlaySubsystem {
    ScreenOverlaySubsystem::new(
        id,
        id.default_label(),
        ScreenOverlaySubsystemPhase::Waiting,
        state_label,
        detail,
        None,
    )
}

pub(crate) fn subsystem_run(
    id: ScreenOverlaySubsystemId,
    state_label: impl Into<String>,
    detail: impl Into<String>,
    progress_01: Option<f32>,
) -> ScreenOverlaySubsystem {
    ScreenOverlaySubsystem::new(
        id,
        id.default_label(),
        ScreenOverlaySubsystemPhase::Running,
        state_label,
        detail,
        progress_01.map(ScreenOverlayProgress::percent),
    )
}

pub(crate) fn subsystem_ready(
    id: ScreenOverlaySubsystemId,
    state_label: impl Into<String>,
    detail: impl Into<String>,
) -> ScreenOverlaySubsystem {
    ScreenOverlaySubsystem::new(
        id,
        id.default_label(),
        ScreenOverlaySubsystemPhase::Ready,
        state_label,
        detail,
        Some(ScreenOverlayProgress::percent(1.0)),
    )
}

pub(crate) fn subsystem_failed(
    id: ScreenOverlaySubsystemId,
    state_label: impl Into<String>,
    detail: impl Into<String>,
) -> ScreenOverlaySubsystem {
    ScreenOverlaySubsystem::new(
        id,
        id.default_label(),
        ScreenOverlaySubsystemPhase::Failed,
        state_label,
        detail,
        None,
    )
}
