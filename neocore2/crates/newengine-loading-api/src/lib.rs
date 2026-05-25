#![forbid(unsafe_op_in_unsafe_fn)]

use serde::{Deserialize, Serialize};

pub mod bootstrap_ui;

/// Engine-facing loading-screen gateway id.
///
/// Runtime/platform code calls this stable facade. The current implementation is
/// an engine-owned native loading shell bridge, while future profile/plugin
/// providers may expose their own `loading.api` backend behind the same gateway.
pub const ENGINE_LOADING_SERVICE_ID: &str = "engine.loading";

/// First-party provider service id for loading shell providers.
pub const LOADING_SERVICE_ID: &str = "loading.api";
pub const LOADING_BACKEND_CAPABILITY_ID: &str = "loading.backend";

pub const LOADING_SERVICE_METHOD_INFO: &str = newengine_service_api::SERVICE_METHOD_INFO_JSON;
pub const LOADING_SERVICE_METHOD_INVOKE: &str = newengine_service_api::SERVICE_METHOD_INVOKE_JSON;
pub const LOADING_SERVICE_METHOD_SHUTDOWN_V1: &str = newengine_service_api::SERVICE_METHOD_SHUTDOWN_V1;
pub const LOADING_SERVICE_METHOD_SNAPSHOT_JSON_V1: &str = "snapshot_json_v1";
pub const LOADING_SERVICE_METHOD_PUBLISH_JSON_V1: &str = "publish_json_v1";
pub const LOADING_SERVICE_METHOD_PUBLISH_STATUS_JSON_V1: &str = "publish_status_json_v1";
pub const ENGINE_LOADING_STATUS_TOPIC_V1: &str = "engine.loading.status.v1";

pub const LOADING_REQUIRED_METHODS_V1: &[&str] = &[
    LOADING_SERVICE_METHOD_INFO,
    LOADING_SERVICE_METHOD_INVOKE,
    LOADING_SERVICE_METHOD_SHUTDOWN_V1,
    LOADING_SERVICE_METHOD_SNAPSHOT_JSON_V1,
];

pub const LOADING_BACKEND_SERVICE_SPEC: newengine_service_api::BackendServiceSpec =
    newengine_service_api::BackendServiceSpec::new(
        "loading",
        ENGINE_LOADING_SERVICE_ID,
        LOADING_SERVICE_ID,
        LOADING_BACKEND_CAPABILITY_ID,
    );

pub const LOADING_RUNTIME_CONTRACT_SPEC: newengine_service_api::RuntimeServiceContractSpec =
    newengine_service_api::RuntimeServiceContractSpec::new(
        ENGINE_LOADING_SERVICE_ID,
        "newengine.loading-api >= 0.1.x",
        LOADING_REQUIRED_METHODS_V1,
    );

/// Loading is an always-helpful diagnostic domain, but it must not make headless
/// or test runs fatal unless a strict profile explicitly requires it.
pub const LOADING_RUNTIME_REQUIREMENT_SPEC: newengine_service_api::RuntimeServiceRequirementSpec =
    newengine_service_api::RuntimeServiceRequirementSpec::new(
        LOADING_RUNTIME_CONTRACT_SPEC,
        Some(LOADING_BACKEND_CAPABILITY_ID),
        Some("NEWENGINE_REQUIRE_LOADING_BACKEND"),
    );

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadingServiceInfo {
    pub protocol: String,
    #[serde(default)]
    pub features: Vec<String>,
    #[serde(default)]
    pub methods: Vec<String>,
}

impl Default for LoadingServiceInfo {
    #[inline]
    fn default() -> Self {
        Self {
            protocol: "newengine.loading-api/v1".to_owned(),
            features: vec![
                "engine-owned-native-shell".to_owned(),
                "shared-snapshot".to_owned(),
                "independent-visual-clock".to_owned(),
                "subsystem-stage-projection".to_owned(),
                "task-event-projection".to_owned(),
                "cooperative-task-control".to_owned(),
            ],
            methods: LOADING_REQUIRED_METHODS_V1
                .iter()
                .map(|it| (*it).to_owned())
                .chain(std::iter::once(LOADING_SERVICE_METHOD_PUBLISH_JSON_V1.to_owned()))
                .chain(std::iter::once(LOADING_SERVICE_METHOD_PUBLISH_STATUS_JSON_V1.to_owned()))
                .collect(),
        }
    }
}


#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LoadingStatusPhase {
    Waiting,
    Running,
    Ready,
    Degraded,
    Failed,
}

impl LoadingStatusPhase {
    #[inline]
    pub const fn as_subsystem_phase(self) -> LoadingSubsystemPhase {
        match self {
            Self::Waiting => LoadingSubsystemPhase::Waiting,
            Self::Running => LoadingSubsystemPhase::Running,
            Self::Ready => LoadingSubsystemPhase::Ready,
            Self::Degraded => LoadingSubsystemPhase::Degraded,
            Self::Failed => LoadingSubsystemPhase::Failed,
        }
    }

    #[inline]
    pub const fn state_label(self) -> &'static str {
        match self {
            Self::Waiting => "WAIT",
            Self::Running => "RUNNING",
            Self::Ready => "READY",
            Self::Degraded => "DEGRADED",
            Self::Failed => "ERR",
        }
    }
}

impl Default for LoadingStatusPhase {
    #[inline]
    fn default() -> Self { Self::Running }
}

/// Generic loading/status event published through the engine bus.
///
/// Producers describe what they are doing; renderers, asset providers and
/// runtime gates do not paint UI directly. `engine.loading` projects this DTO
/// into the current loading surface snapshot.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LoadingStatusEvent {
    pub source: String,
    pub domain: String,
    pub subsystem_id: String,
    pub subsystem_label: String,
    pub phase: LoadingStatusPhase,
    pub state_label: String,
    pub title: String,
    pub status: String,
    pub detail: String,
    pub progress_01: f32,
    #[serde(default)]
    pub terminal: bool,
}

impl LoadingStatusEvent {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        source: impl Into<String>,
        domain: impl Into<String>,
        subsystem_id: impl Into<String>,
        subsystem_label: impl Into<String>,
        phase: LoadingStatusPhase,
        title: impl Into<String>,
        status: impl Into<String>,
        detail: impl Into<String>,
        progress_01: f32,
    ) -> Self {
        let phase_label = phase.state_label();
        Self {
            source: source.into(),
            domain: domain.into(),
            subsystem_id: subsystem_id.into(),
            subsystem_label: subsystem_label.into(),
            phase,
            state_label: phase_label.to_owned(),
            title: title.into(),
            status: status.into(),
            detail: detail.into(),
            progress_01: progress_01.clamp(0.0, 1.0),
            terminal: matches!(phase, LoadingStatusPhase::Ready | LoadingStatusPhase::Failed),
        }
    }

    #[inline]
    pub fn with_state_label(mut self, state_label: impl Into<String>) -> Self {
        self.state_label = state_label.into();
        self
    }

    #[inline]
    pub fn with_terminal(mut self, terminal: bool) -> Self {
        self.terminal = terminal;
        self
    }

    pub fn to_subsystem_snapshot(&self) -> LoadingSubsystemSnapshot {
        LoadingSubsystemSnapshot::new(
            self.subsystem_id.as_str(),
            self.subsystem_label.as_str(),
            self.phase.as_subsystem_phase(),
            self.state_label.as_str(),
            self.detail.as_str(),
            Some(self.progress_01),
        )
    }
}


/// Host/event-bus topic for every engine task lifecycle update.
///
/// This is deliberately generic: plugin lifecycle calls, service calls, CPU jobs,
/// asset worker passes and renderer bootstrap phases can all publish the same
/// shape. Loading/UI/debug tools can then build a live task view without knowing
/// which subsystem performs the work.
pub const ENGINE_TASK_EVENT_TOPIC_V1: &str = "engine.task.event.v1";

/// Host/event-bus topic for cooperative task control requests.
///
/// A task may ignore a control action only when it explicitly advertised that the
/// action is unsupported. Otherwise cancellation and pause/resume are cooperative
/// and must be observed at task-owned checkpoints.
pub const ENGINE_TASK_CONTROL_TOPIC_V1: &str = "engine.task.control.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EngineTaskPhase {
    Scheduled,
    Running,
    Blocked,
    PauseRequested,
    Paused,
    ResumeRequested,
    CancelRequested,
    Cancelled,
    Completed,
    Failed,
}

impl EngineTaskPhase {
    #[inline]
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Cancelled | Self::Completed | Self::Failed)
    }

    #[inline]
    pub const fn loading_phase(self) -> LoadingStatusPhase {
        match self {
            Self::Scheduled | Self::PauseRequested | Self::ResumeRequested => LoadingStatusPhase::Waiting,
            Self::Running | Self::Blocked | Self::Paused | Self::CancelRequested => LoadingStatusPhase::Running,
            Self::Completed | Self::Cancelled => LoadingStatusPhase::Ready,
            Self::Failed => LoadingStatusPhase::Failed,
        }
    }

    #[inline]
    pub const fn state_label(self) -> &'static str {
        match self {
            Self::Scheduled => "QUEUED",
            Self::Running => "RUNNING",
            Self::Blocked => "BLOCKED",
            Self::PauseRequested => "PAUSE",
            Self::Paused => "PAUSED",
            Self::ResumeRequested => "RESUME",
            Self::CancelRequested => "CANCEL",
            Self::Cancelled => "CANCELLED",
            Self::Completed => "DONE",
            Self::Failed => "ERR",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EngineTaskControlAction {
    Pause,
    Resume,
    Cancel,
}

impl EngineTaskControlAction {
    #[inline]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pause => "pause",
            Self::Resume => "resume",
            Self::Cancel => "cancel",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EngineTaskControlEvent {
    pub task_id: String,
    pub action: EngineTaskControlAction,
    #[serde(default)]
    pub reason: String,
    #[serde(default)]
    pub source: String,
}

impl EngineTaskControlEvent {
    #[inline]
    pub fn new(task_id: impl Into<String>, action: EngineTaskControlAction) -> Self {
        Self {
            task_id: task_id.into(),
            action,
            reason: String::new(),
            source: "engine.task.control".to_owned(),
        }
    }

    #[inline]
    pub fn with_reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = reason.into();
        self
    }

    #[inline]
    pub fn with_source(mut self, source: impl Into<String>) -> Self {
        self.source = source.into();
        self
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EngineTaskEvent {
    pub task_id: String,
    #[serde(default)]
    pub parent_task_id: Option<String>,
    pub source: String,
    pub owner: String,
    pub category: String,
    pub name: String,
    pub lane: String,
    pub phase: EngineTaskPhase,
    pub state_label: String,
    pub status: String,
    pub detail: String,
    #[serde(default)]
    pub progress_01: Option<f32>,
    pub can_pause: bool,
    pub can_cancel: bool,
    pub terminal: bool,
}

impl EngineTaskEvent {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        task_id: impl Into<String>,
        source: impl Into<String>,
        owner: impl Into<String>,
        category: impl Into<String>,
        name: impl Into<String>,
        lane: impl Into<String>,
        phase: EngineTaskPhase,
        status: impl Into<String>,
        detail: impl Into<String>,
    ) -> Self {
        let phase_label = phase.state_label();
        Self {
            task_id: task_id.into(),
            parent_task_id: None,
            source: source.into(),
            owner: owner.into(),
            category: category.into(),
            name: name.into(),
            lane: lane.into(),
            phase,
            state_label: phase_label.to_owned(),
            status: status.into(),
            detail: detail.into(),
            progress_01: None,
            can_pause: false,
            can_cancel: false,
            terminal: phase.is_terminal(),
        }
    }

    #[inline]
    pub fn with_parent_task_id(mut self, parent_task_id: impl Into<String>) -> Self {
        self.parent_task_id = Some(parent_task_id.into());
        self
    }

    #[inline]
    pub fn with_progress(mut self, progress_01: f32) -> Self {
        self.progress_01 = Some(progress_01.clamp(0.0, 1.0));
        self
    }

    #[inline]
    pub fn with_controls(mut self, can_pause: bool, can_cancel: bool) -> Self {
        self.can_pause = can_pause;
        self.can_cancel = can_cancel;
        self
    }

    #[inline]
    pub fn with_state_label(mut self, state_label: impl Into<String>) -> Self {
        self.state_label = state_label.into();
        self
    }

    #[inline]
    pub fn with_terminal(mut self, terminal: bool) -> Self {
        self.terminal = terminal;
        self
    }

    pub fn to_loading_status_event(&self) -> LoadingStatusEvent {
        let progress = self.progress_01.unwrap_or_else(|| {
            if self.phase.is_terminal() { 1.0 } else { 0.0 }
        });
        LoadingStatusEvent::new(
            self.source.as_str(),
            self.category.as_str(),
            self.category.as_str(),
            self.category.to_ascii_uppercase(),
            self.phase.loading_phase(),
            "NORTH STAR ENGINE // TASKS",
            self.status.as_str(),
            self.detail.as_str(),
            progress,
        )
        .with_state_label(self.state_label.as_str())
        .with_terminal(false)
    }

    pub fn to_subsystem_snapshot(&self) -> LoadingSubsystemSnapshot {
        LoadingSubsystemSnapshot::new(
            self.category.as_str(),
            self.category.to_ascii_uppercase(),
            self.phase.loading_phase().as_subsystem_phase(),
            self.state_label.as_str(),
            self.detail.as_str(),
            self.progress_01,
        )
    }
}


#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LoadingSubsystemPhase {
    Waiting,
    Running,
    Ready,
    Degraded,
    Failed,
}

impl Default for LoadingSubsystemPhase {
    #[inline]
    fn default() -> Self {
        Self::Waiting
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LoadingSubsystemSnapshot {
    pub id: String,
    pub label: String,
    pub phase: LoadingSubsystemPhase,
    pub state_label: String,
    pub detail: String,
    pub progress_01: Option<f32>,
}

impl LoadingSubsystemSnapshot {
    #[inline]
    pub fn new(
        id: impl Into<String>,
        label: impl Into<String>,
        phase: LoadingSubsystemPhase,
        state_label: impl Into<String>,
        detail: impl Into<String>,
        progress_01: Option<f32>,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            phase,
            state_label: state_label.into(),
            detail: detail.into(),
            progress_01: progress_01.map(|v| v.clamp(0.0, 1.0)),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LoadingScreenSnapshot {
    pub active: bool,
    pub title: String,
    pub status: String,
    pub detail: String,
    pub progress_01: f32,
    pub spinner_phase: u32,
    pub source: String,
    #[serde(default)]
    pub provider: String,
    #[serde(default)]
    pub view_json: String,
    #[serde(default)]
    pub subsystems: Vec<LoadingSubsystemSnapshot>,
}

impl Default for LoadingScreenSnapshot {
    #[inline]
    fn default() -> Self {
        Self {
            active: false,
            title: "NORTH STAR ENGINE // BOOTSTRAP".to_owned(),
            status: "Preparing runtime...".to_owned(),
            detail: "The native loading shell is waiting for startup telemetry.".to_owned(),
            progress_01: 0.0,
            spinner_phase: 0,
            source: "engine.loading".to_owned(),
            provider: "native-shell".to_owned(),
            view_json: String::new(),
            subsystems: Vec::new(),
        }
    }
}

impl LoadingScreenSnapshot {
    #[inline]
    pub fn inactive() -> Self {
        Self::default()
    }

    #[inline]
    pub fn normalize(mut self) -> Self {
        self.progress_01 = self.progress_01.clamp(0.0, 1.0);
        for subsystem in &mut self.subsystems {
            subsystem.progress_01 = subsystem.progress_01.map(|v| v.clamp(0.0, 1.0));
        }
        self
    }
}
