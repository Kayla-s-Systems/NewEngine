#![forbid(unsafe_op_in_unsafe_fn)]

/// Real, engine-owned startup phase used by loading diagnostics and engine.ui projections.
///
/// This is deliberately hosted in `newengine-core`: platform/runtime/editor layers
/// may observe it, but they must not create a second lifecycle truth beside the
/// core FSM.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngineStartupPhase {
    Idle,
    SystemInit,
    ApiContracts,
    GameInit,
    ModuleOrder,
    ModuleInit,
    StartupGraph,
    RuntimePlugins,
    PluginStart,
    ServiceContracts,
    ReadinessEvents,
    Running,
    Faulted,
}

impl EngineStartupPhase {
    #[inline]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::SystemInit => "system-init",
            Self::ApiContracts => "api-contracts",
            Self::GameInit => "game-init",
            Self::ModuleOrder => "module-order",
            Self::ModuleInit => "module-init",
            Self::StartupGraph => "startup-graph",
            Self::RuntimePlugins => "runtime-plugins",
            Self::PluginStart => "plugin-start",
            Self::ServiceContracts => "service-contracts",
            Self::ReadinessEvents => "readiness-events",
            Self::Running => "running",
            Self::Faulted => "faulted",
        }
    }

    #[inline]
    pub const fn human_label(self) -> &'static str {
        match self {
            Self::Idle => "IDLE",
            Self::SystemInit => "INIT SYSTEM",
            Self::ApiContracts => "API CHECK",
            Self::GameInit => "INIT GAME",
            Self::ModuleOrder => "MODULE ORDER",
            Self::ModuleInit => "MODULE INIT",
            Self::StartupGraph => "STARTUP GRAPH",
            Self::RuntimePlugins => "PLUGINS",
            Self::PluginStart => "PLUGIN START",
            Self::ServiceContracts => "SERVICE CHECK",
            Self::ReadinessEvents => "READINESS",
            Self::Running => "RUNNING",
            Self::Faulted => "FAULTED",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngineStartupSystemPhase {
    Waiting,
    Running,
    Ready,
    Degraded,
    Failed,
}

impl EngineStartupSystemPhase {
    #[inline]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Waiting => "waiting",
            Self::Running => "running",
            Self::Ready => "ready",
            Self::Degraded => "degraded",
            Self::Failed => "failed",
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

#[derive(Debug, Clone, PartialEq)]
pub struct EngineStartupSystemStatus {
    pub id: String,
    pub label: String,
    pub phase: EngineStartupSystemPhase,
    pub state_label: String,
    pub detail: String,
    pub progress_01: Option<f32>,
}

impl EngineStartupSystemStatus {
    #[inline]
    pub fn new(
        id: impl Into<String>,
        label: impl Into<String>,
        phase: EngineStartupSystemPhase,
        state_label: impl Into<String>,
        detail: impl Into<String>,
        progress_01: Option<f32>,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            phase,
            state_label: normalize(state_label.into(), phase.state_label()),
            detail: detail.into(),
            progress_01: progress_01.map(|v| v.clamp(0.0, 1.0)),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct EngineStartupSnapshot {
    pub active: bool,
    pub terminal: bool,
    pub phase: EngineStartupPhase,
    pub run_state: &'static str,
    pub status: String,
    pub detail: String,
    pub progress_01: f32,
    pub current_module: Option<String>,
    pub module_index: usize,
    pub module_total: usize,
    pub plugin_count: usize,
    pub systems: Vec<EngineStartupSystemStatus>,
    pub error: Option<String>,
}

impl EngineStartupSnapshot {
    #[inline]
    pub fn idle(run_state: &'static str) -> Self {
        Self {
            active: false,
            terminal: false,
            phase: EngineStartupPhase::Idle,
            run_state,
            status: "Waiting for startup.".to_owned(),
            detail: "Engine startup has not been requested yet.".to_owned(),
            progress_01: 0.0,
            current_module: None,
            module_index: 0,
            module_total: 0,
            plugin_count: 0,
            systems: Vec::new(),
            error: None,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn running(
        phase: EngineStartupPhase,
        run_state: &'static str,
        status: impl Into<String>,
        detail: impl Into<String>,
        progress_01: f32,
        current_module: Option<String>,
        module_index: usize,
        module_total: usize,
        plugin_count: usize,
        systems: Vec<EngineStartupSystemStatus>,
    ) -> Self {
        Self {
            active: true,
            terminal: false,
            phase,
            run_state,
            status: normalize(status.into(), phase.human_label()),
            detail: normalize(detail.into(), "Engine startup is progressing."),
            progress_01: progress_01.clamp(0.0, 0.999),
            current_module,
            module_index,
            module_total,
            plugin_count,
            systems,
            error: None,
        }
    }

    pub fn complete(run_state: &'static str, module_total: usize, plugin_count: usize) -> Self {
        Self {
            active: false,
            terminal: true,
            phase: EngineStartupPhase::Running,
            run_state,
            status: "Engine runtime ready.".to_owned(),
            detail: "Core FSM reached running; frame loop may accept playable-world handoff.".to_owned(),
            progress_01: 1.0,
            current_module: None,
            module_index: module_total,
            module_total,
            plugin_count,
            systems: Vec::new(),
            error: None,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn failed(
        phase: EngineStartupPhase,
        run_state: &'static str,
        status: impl Into<String>,
        detail: impl Into<String>,
        progress_01: f32,
        current_module: Option<String>,
        module_index: usize,
        module_total: usize,
        plugin_count: usize,
        error: impl Into<String>,
    ) -> Self {
        let error = error.into();
        Self {
            active: true,
            terminal: true,
            phase,
            run_state,
            status: normalize(status.into(), "Startup failed."),
            detail: normalize(detail.into(), error.as_str()),
            progress_01: progress_01.clamp(0.0, 1.0),
            current_module,
            module_index,
            module_total,
            plugin_count,
            systems: vec![EngineStartupSystemStatus::new(
                "diagnostics",
                "DIAGNOSTICS",
                EngineStartupSystemPhase::Failed,
                "ERR",
                error.as_str(),
                Some(progress_01),
            )],
            error: Some(error),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngineStartupStepPhase {
    Idle,
    EnterSystemInit,
    ValidateApiContracts,
    EnterGameInit,
    PrepareModuleOrder,
    InitModules,
    StartupGraphInitial,
    LoadRuntimePlugins,
    StartPlugins,
    ValidateRuntimeServiceContracts,
    DispatchReadiness,
    EnterRunning,
    Complete,
}

impl EngineStartupStepPhase {
    #[inline]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::EnterSystemInit => "enter-system-init",
            Self::ValidateApiContracts => "validate-api-contracts",
            Self::EnterGameInit => "enter-game-init",
            Self::PrepareModuleOrder => "prepare-module-order",
            Self::InitModules => "init-modules",
            Self::StartupGraphInitial => "startup-graph-initial",
            Self::LoadRuntimePlugins => "load-runtime-plugins",
            Self::StartPlugins => "start-plugins",
            Self::ValidateRuntimeServiceContracts => "validate-runtime-service-contracts",
            Self::DispatchReadiness => "dispatch-readiness",
            Self::EnterRunning => "enter-running",
            Self::Complete => "complete",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineIncrementalStartupState {
    pub phase: EngineStartupStepPhase,
    pub index: usize,
    pub initialized: usize,
    pub module_total: usize,
}

impl Default for EngineIncrementalStartupState {
    fn default() -> Self {
        Self {
            phase: EngineStartupStepPhase::Idle,
            index: 0,
            initialized: 0,
            module_total: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct EngineStartupStepOutcome {
    pub finished: bool,
    pub snapshot: EngineStartupSnapshot,
}

impl EngineStartupStepOutcome {
    #[inline]
    pub fn running(snapshot: EngineStartupSnapshot) -> Self {
        Self { finished: false, snapshot }
    }

    #[inline]
    pub fn complete(snapshot: EngineStartupSnapshot) -> Self {
        Self { finished: true, snapshot }
    }
}

fn normalize(value: String, fallback: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() { fallback.to_owned() } else { trimmed.to_owned() }
}
