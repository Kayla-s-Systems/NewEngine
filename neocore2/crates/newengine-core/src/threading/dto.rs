use serde::{Deserialize, Serialize};

use crate::task_core::{TaskLane, TaskPriority as CoreTaskPriority, TaskRequest};
use newengine_loading_api::EngineTaskPhase;

use super::config::THREADING_BACKEND_CAPABILITY_ID;

/// DTO-first CPU task envelope for engine.threading callers.
///
/// The payload remains opaque to the core. Domain modules own decoding and
/// semantic interpretation; the threading core owns execution, budget, control
/// and diagnostics.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct CpuTaskDto {
    pub task_id: String,
    pub domain: String,
    pub priority: CpuTaskPriority,
    pub budget_hint_ms: u32,
    pub capability: String,
    pub payload: Vec<u8>,
}

impl Default for CpuTaskDto {
    fn default() -> Self {
        Self {
            task_id: String::new(),
            domain: "engine.threading".to_owned(),
            priority: CpuTaskPriority::Normal,
            budget_hint_ms: 0,
            capability: THREADING_BACKEND_CAPABILITY_ID.to_owned(),
            payload: Vec::new(),
        }
    }
}

impl CpuTaskDto {
    #[inline]
    pub fn new(domain: impl Into<String>) -> Self {
        Self {
            domain: domain.into(),
            ..Default::default()
        }
    }

    #[inline]
    pub fn with_task_id(mut self, task_id: impl Into<String>) -> Self {
        self.task_id = task_id.into();
        self
    }

    #[inline]
    pub fn with_priority(mut self, priority: CpuTaskPriority) -> Self {
        self.priority = priority;
        self
    }

    #[inline]
    pub fn with_budget_hint_ms(mut self, budget_hint_ms: u32) -> Self {
        self.budget_hint_ms = budget_hint_ms;
        self
    }

    #[inline]
    pub fn with_capability(mut self, capability: impl Into<String>) -> Self {
        self.capability = capability.into();
        self
    }

    #[inline]
    pub fn with_payload(mut self, payload: impl Into<Vec<u8>>) -> Self {
        self.payload = payload.into();
        self
    }

    pub(crate) fn to_task_request(&self) -> TaskRequest {
        let domain = canonical_domain(self.domain.as_str());
        let mut request = TaskRequest::new("cpu-task")
            .with_source("engine.threading")
            .with_owner(domain)
            .with_category("cpu-task")
            .with_lane(domain_lane(self.domain.as_str()))
            .with_priority(self.priority.into_task_priority())
            .with_task_domain(domain)
            .with_task_pass("cpu-work")
            .pausable(false)
            .cancellable(true);

        if !self.task_id.trim().is_empty() {
            request = request.with_task_id(self.task_id.trim().to_owned());
        }
        if !self.capability.trim().is_empty() {
            request =
                request.with_dependency_group(format!("capability:{}", self.capability.trim()));
        }
        request
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum CpuTaskPriority {
    Realtime,
    High,
    #[default]
    Normal,
    Low,
    Idle,
}

impl CpuTaskPriority {
    #[inline]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Realtime => "realtime",
            Self::High => "high",
            Self::Normal => "normal",
            Self::Low => "low",
            Self::Idle => "idle",
        }
    }

    #[inline]
    pub const fn into_task_priority(self) -> CoreTaskPriority {
        match self {
            Self::Realtime => CoreTaskPriority::Critical,
            Self::High => CoreTaskPriority::Interactive,
            Self::Normal => CoreTaskPriority::Normal,
            Self::Low | Self::Idle => CoreTaskPriority::Background,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskStatus {
    #[default]
    Scheduled,
    Running,
    Completed,
    Cancelled,
    Failed,
}

impl TaskStatus {
    #[inline]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Scheduled => "scheduled",
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Cancelled => "cancelled",
            Self::Failed => "failed",
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct CpuTaskResultDto {
    pub task_id: String,
    pub status: TaskStatus,
    pub cpu_time_ns: u64,
    pub output: Vec<u8>,
}

pub(crate) fn task_status_from_phase(phase: EngineTaskPhase) -> TaskStatus {
    match phase {
        EngineTaskPhase::Completed => TaskStatus::Completed,
        EngineTaskPhase::Cancelled => TaskStatus::Cancelled,
        EngineTaskPhase::Failed => TaskStatus::Failed,
        EngineTaskPhase::Running | EngineTaskPhase::Paused => TaskStatus::Running,
        _ => TaskStatus::Scheduled,
    }
}

fn canonical_domain(value: &str) -> &'static str {
    match value.trim() {
        "ai" | "engine.ai" => "engine.ai",
        "assets" | "asset" | "engine.assets" => "engine.assets",
        "physics" | "engine.physics" => "engine.physics",
        "streaming" | "engine.streaming" | "engine.world.streaming" => "engine.streaming",
        "render" | "render-prep" | "engine.render" | "engine.render.prep" => "engine.render",
        "plugin" | "plugins" | "engine.plugin" => "engine.plugin",
        "simulation" | "engine.simulation" => "engine.simulation",
        _ => "engine.threading",
    }
}

fn domain_lane(value: &str) -> TaskLane {
    match canonical_domain(value) {
        "engine.ai" | "engine.physics" | "engine.simulation" => TaskLane::Simulation,
        "engine.render" => TaskLane::RenderPrep,
        "engine.assets" => TaskLane::AssetIo,
        "engine.streaming" => TaskLane::Streaming,
        "engine.plugin" => TaskLane::Plugin,
        _ => TaskLane::Background,
    }
}
