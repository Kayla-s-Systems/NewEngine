#![forbid(unsafe_op_in_unsafe_fn)]

use serde::{Deserialize, Serialize};

pub use newengine_loading_api::{
    EngineTaskControlAction, EngineTaskControlEvent, EngineTaskEvent, EngineTaskPhase,
    ENGINE_TASK_CONTROL_TOPIC_V1, ENGINE_TASK_EVENT_TOPIC_V1,
};

pub const ENGINE_JOBS_SERVICE_ID: &str = "engine.jobs";
pub const JOBS_SERVICE_ID: &str = "jobs.api";
pub const JOBS_BACKEND_CAPABILITY_ID: &str = "jobs.backend";
pub const JOBS_RUNTIME_CONTRACT: &str = "newengine.jobs.runtime.v1";

/// Canonical topic for engine.jobs-compatible job lifecycle envelopes.
///
/// `ENGINE_TASK_EVENT_TOPIC_V1` remains the UI/loading projection topic. This
/// topic carries the stricter job-authority envelope for diagnostics, profiler
/// and CI-visible work streams.
pub const ENGINE_JOB_EVENT_TOPIC_V1: &str = "engine.jobs.event.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum JobExecutorKind {
    EngineWorker,
    PluginHostBridge,
    ToolRunner,
    SimulationInternalParallelism,
    RuntimeWatchdog,
    ExternalProvider,
}

impl JobExecutorKind {
    #[inline]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::EngineWorker => "engine-worker",
            Self::PluginHostBridge => "plugin-host-bridge",
            Self::ToolRunner => "tool-runner",
            Self::SimulationInternalParallelism => "simulation-internal-parallelism",
            Self::RuntimeWatchdog => "runtime-watchdog",
            Self::ExternalProvider => "external-provider",
        }
    }
}

impl Default for JobExecutorKind {
    #[inline]
    fn default() -> Self { Self::EngineWorker }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct JobAuthorityV1 {
    pub gateway: String,
    pub provider: String,
    pub contract: String,
    pub event_topic: String,
    pub control_topic: String,
    pub job_event_topic: String,
    pub identity_required: bool,
    pub invisible_work_allowed: bool,
}

impl Default for JobAuthorityV1 {
    fn default() -> Self {
        Self {
            gateway: ENGINE_JOBS_SERVICE_ID.to_owned(),
            provider: JOBS_SERVICE_ID.to_owned(),
            contract: JOBS_RUNTIME_CONTRACT.to_owned(),
            event_topic: ENGINE_TASK_EVENT_TOPIC_V1.to_owned(),
            control_topic: ENGINE_TASK_CONTROL_TOPIC_V1.to_owned(),
            job_event_topic: ENGINE_JOB_EVENT_TOPIC_V1.to_owned(),
            identity_required: true,
            invisible_work_allowed: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct EngineJobEventV1 {
    pub schema: String,
    pub authority: JobAuthorityV1,
    pub executor: JobExecutorKind,
    pub semantic_owner: String,
    pub event: EngineTaskEvent,
}

impl Default for EngineJobEventV1 {
    fn default() -> Self {
        Self {
            schema: "newengine.jobs.event.v1".to_owned(),
            authority: JobAuthorityV1::default(),
            executor: JobExecutorKind::EngineWorker,
            semantic_owner: "runtime-work".to_owned(),
            event: EngineTaskEvent::new(
                "job.unknown",
                ENGINE_JOBS_SERVICE_ID,
                ENGINE_JOBS_SERVICE_ID,
                "runtime",
                "unknown-job",
                "runtime",
                EngineTaskPhase::Scheduled,
                "Job scheduled",
                "Job event was created without a concrete producer.",
            ),
        }
    }
}

impl EngineJobEventV1 {
    #[inline]
    pub fn new(event: EngineTaskEvent, executor: JobExecutorKind, semantic_owner: impl Into<String>) -> Self {
        Self {
            executor,
            semantic_owner: semantic_owner.into(),
            event,
            ..Default::default()
        }
    }

    #[inline]
    pub fn into_task_event(self) -> EngineTaskEvent {
        self.event
    }
}

pub mod jobs_method {
    pub const INFO_JSON: &str = newengine_service_api::SERVICE_METHOD_INFO_JSON;
    pub const INVOKE_JSON: &str = newengine_service_api::SERVICE_METHOD_INVOKE_JSON;
    pub const SHUTDOWN_V1: &str = newengine_service_api::SERVICE_METHOD_SHUTDOWN_V1;
    pub const START_V1: &str = "job.start_v1";
    pub const CANCEL_V1: &str = "job.cancel_v1";
    pub const PAUSE_V1: &str = "job.pause_v1";
    pub const RESUME_V1: &str = "job.resume_v1";
    pub const STATUS_JSON_V1: &str = "job.status_json_v1";
    pub const PROGRESS_EVENT_V1: &str = "job.progress_event_v1";
    pub const TRACE_JSON_V1: &str = "job.trace_json_v1";
    pub const SNAPSHOT_JSON_V1: &str = "job.snapshot_json_v1";
}

pub const JOBS_SERVICE_METHODS: &[&str] = &[
    jobs_method::INFO_JSON,
    jobs_method::INVOKE_JSON,
    jobs_method::SHUTDOWN_V1,
    jobs_method::START_V1,
    jobs_method::CANCEL_V1,
    jobs_method::PAUSE_V1,
    jobs_method::RESUME_V1,
    jobs_method::STATUS_JSON_V1,
    jobs_method::PROGRESS_EVENT_V1,
    jobs_method::TRACE_JSON_V1,
    jobs_method::SNAPSHOT_JSON_V1,
];

pub const JOBS_BACKEND_SERVICE_SPEC: newengine_service_api::BackendServiceSpec =
    newengine_service_api::BackendServiceSpec::new(
        "jobs",
        ENGINE_JOBS_SERVICE_ID,
        JOBS_SERVICE_ID,
        JOBS_BACKEND_CAPABILITY_ID,
    );

pub const JOBS_RUNTIME_CONTRACT_SPEC: newengine_service_api::RuntimeServiceContractSpec =
    newengine_service_api::RuntimeServiceContractSpec::new(
        ENGINE_JOBS_SERVICE_ID,
        JOBS_RUNTIME_CONTRACT,
        JOBS_SERVICE_METHODS,
    );

pub const JOBS_RUNTIME_REQUIREMENT_SPEC: newengine_service_api::RuntimeServiceRequirementSpec =
    newengine_service_api::RuntimeServiceRequirementSpec::new(
        JOBS_RUNTIME_CONTRACT_SPEC,
        Some(JOBS_BACKEND_CAPABILITY_ID),
        Some("NEWENGINE_REQUIRE_JOBS_BACKEND"),
    );

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct JobsServiceInfoV1 {
    pub service_id: String,
    pub gateway: String,
    pub provider: String,
    pub contract: String,
    pub methods: Vec<String>,
    pub event_topic: String,
    pub control_topic: String,
    pub job_event_topic: String,
    pub authority: JobAuthorityV1,
    pub cooperative_control: bool,
}

impl Default for JobsServiceInfoV1 {
    fn default() -> Self {
        Self {
            service_id: JOBS_SERVICE_ID.to_owned(),
            gateway: ENGINE_JOBS_SERVICE_ID.to_owned(),
            provider: "EngineOwnedJobsProvider".to_owned(),
            contract: JOBS_RUNTIME_CONTRACT.to_owned(),
            methods: JOBS_SERVICE_METHODS.iter().map(|m| (*m).to_owned()).collect(),
            event_topic: ENGINE_TASK_EVENT_TOPIC_V1.to_owned(),
            control_topic: ENGINE_TASK_CONTROL_TOPIC_V1.to_owned(),
            job_event_topic: ENGINE_JOB_EVENT_TOPIC_V1.to_owned(),
            authority: JobAuthorityV1::default(),
            cooperative_control: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct JobIdRequestV1 {
    pub job_id: String,
    pub reason: String,
    pub source: String,
}

impl Default for JobIdRequestV1 {
    fn default() -> Self {
        Self { job_id: String::new(), reason: String::new(), source: "engine.jobs".to_owned() }
    }
}

impl JobIdRequestV1 {
    pub fn control_event(&self, action: EngineTaskControlAction) -> EngineTaskControlEvent {
        EngineTaskControlEvent::new(self.job_id.clone(), action)
            .with_reason(self.reason.clone())
            .with_source(self.source.clone())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct JobStartRequestV1 {
    pub job_id: String,
    pub name: String,
    pub owner: String,
    pub category: String,
    pub lane: String,
    pub can_pause: bool,
    pub can_cancel: bool,
}

impl Default for JobStartRequestV1 {
    fn default() -> Self {
        Self {
            job_id: String::new(),
            name: "external-job".to_owned(),
            owner: "engine.jobs".to_owned(),
            category: "external".to_owned(),
            lane: "external".to_owned(),
            can_pause: false,
            can_cancel: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct JobProgressEventV1 {
    pub job_id: String,
    pub owner: String,
    pub category: String,
    pub name: String,
    pub lane: String,
    pub status: String,
    pub detail: String,
    pub progress_01: f32,
    pub phase: EngineTaskPhase,
    pub can_pause: bool,
    pub can_cancel: bool,
}

impl Default for JobProgressEventV1 {
    fn default() -> Self {
        Self {
            job_id: String::new(),
            owner: "engine.jobs".to_owned(),
            category: "runtime".to_owned(),
            name: "runtime-job".to_owned(),
            lane: "runtime".to_owned(),
            status: "Job progress".to_owned(),
            detail: String::new(),
            progress_01: 0.0,
            phase: EngineTaskPhase::Running,
            can_pause: false,
            can_cancel: true,
        }
    }
}

impl JobProgressEventV1 {
    pub fn into_task_event(self) -> EngineTaskEvent {
        EngineTaskEvent::new(
            self.job_id,
            "engine.jobs",
            self.owner,
            self.category,
            self.name,
            self.lane,
            self.phase,
            self.status,
            self.detail,
        )
        .with_progress(self.progress_01)
        .with_controls(self.can_pause, self.can_cancel)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct JobStatusJsonV1 {
    pub job_id: String,
    pub name: String,
    pub lane: String,
    pub priority: String,
    pub phase: EngineTaskPhase,
    pub can_pause: bool,
    pub can_cancel: bool,
    pub cancel_requested: bool,
    pub pause_requested: bool,
    pub found: bool,
}

impl Default for JobStatusJsonV1 {
    fn default() -> Self {
        Self {
            job_id: String::new(),
            name: String::new(),
            lane: String::new(),
            priority: String::new(),
            phase: EngineTaskPhase::Scheduled,
            can_pause: false,
            can_cancel: false,
            cancel_requested: false,
            pause_requested: false,
            found: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct JobControlResponseV1 {
    pub job_id: String,
    pub action: String,
    pub accepted: bool,
}

impl Default for JobControlResponseV1 {
    fn default() -> Self { Self { job_id: String::new(), action: String::new(), accepted: false } }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct JobsSnapshotJsonV1 {
    pub worker_threads: usize,
    pub pending_jobs: usize,
    pub running_jobs: usize,
    pub paused_jobs: usize,
    pub submitted_jobs: u64,
    pub completed_jobs: u64,
    pub cancelled_jobs: u64,
    pub panicked_jobs: u64,
}

impl Default for JobsSnapshotJsonV1 {
    fn default() -> Self {
        Self {
            worker_threads: 0,
            pending_jobs: 0,
            running_jobs: 0,
            paused_jobs: 0,
            submitted_jobs: 0,
            completed_jobs: 0,
            cancelled_jobs: 0,
            panicked_jobs: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct JobTraceJsonV1 {
    pub job_id: String,
    pub status: JobStatusJsonV1,
    pub note: String,
}

impl Default for JobTraceJsonV1 {
    fn default() -> Self { Self { job_id: String::new(), status: JobStatusJsonV1::default(), note: String::new() } }
}
