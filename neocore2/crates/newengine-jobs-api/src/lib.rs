#![forbid(unsafe_op_in_unsafe_fn)]
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

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

/// Canonical domain ids used by profiler-visible job passes.
///
/// These strings are not scheduler implementation details. They are the public
/// diagnostic language for the runtime spine: every heavy system should identify
/// which domain owns the work.
pub mod job_domain {
    pub const ENGINE_JOBS: &str = "engine.jobs";
    pub const ENGINE_RENDER: &str = "engine.render";
    pub const ENGINE_RENDER_PREP: &str = "engine.render.prep";
    pub const ENGINE_ASSETS: &str = "engine.assets";
    pub const ENGINE_STREAMING: &str = "engine.streaming";
    pub const ENGINE_WORLD_STREAMING: &str = "engine.world.streaming";
    pub const ENGINE_SIMULATION: &str = "engine.simulation";
    pub const ENGINE_VISIBILITY: &str = "engine.visibility";
    pub const ENGINE_VEGETATION: &str = "engine.vegetation";
    pub const ENGINE_TERRAIN: &str = "engine.terrain";
    pub const ENGINE_ANIMATION: &str = "engine.animation";
    pub const ENGINE_DESTRUCTION: &str = "engine.destruction";
    pub const ENGINE_PARTICLES: &str = "engine.particles";
    pub const ENGINE_SHADER: &str = "engine.shader";
}

/// Canonical job-pass names for production-style domain task graph slices.
///
/// A pass name describes *what kind of work* is happening, independently from
/// which worker executes it. This keeps profiler reports stable when a pass
/// moves from a main-thread barrier to an engine.jobs worker.
pub mod job_pass {
    pub const INPUT: &str = "input";
    pub const CONTROLLERS: &str = "controllers";
    pub const APPLY_INTENTS: &str = "apply-intents";
    pub const PHYSICS: &str = "physics";
    pub const DERIVED: &str = "derived";
    pub const SIM_READ_SNAPSHOT: &str = "sim-read-snapshot";
    pub const SIM_COMMAND_BATCH: &str = "sim-command-batch";
    pub const VISIBILITY: &str = "visibility";
    pub const SCENE_SNAPSHOT: &str = "scene-snapshot";
    pub const SCENE_RENDER_SNAPSHOT: &str = "scene-render-snapshot";
    pub const FEATURE_EXTRACT: &str = "feature-extract";
    pub const RENDER_SUBMIT: &str = "render-submit";
    pub const FRAME_ENVELOPE: &str = "frame-envelope";
    pub const TERRAIN_RENDER_PACKET: &str = "terrain-render-packet";
    pub const TERRAIN_GPU_RESIDENCY: &str = "terrain-gpu-residency";
    pub const WORLD_STREAMING_PLAN: &str = "world-streaming-plan";
    pub const ASSET_IO: &str = "asset-io";
    pub const LISTFILE_DECODE: &str = "listfile-decode";
    pub const SEMANTIC_DTO: &str = "semantic-dto";
    pub const TEXTURE_DECODE: &str = "texture-decode";
    pub const TEXTURE_UPLOAD: &str = "texture-upload";
    pub const SHADER_COMPILE: &str = "shader-compile";
    pub const VEGETATION_BUILD: &str = "vegetation-build";
    pub const ANIMATION_SAMPLING: &str = "animation-sampling";
    pub const DESTRUCTION_BUILD: &str = "destruction-build";
    pub const PARTICLE_SIMULATION: &str = "particle-simulation";
    pub const SERVICE_CALL: &str = "service-call";
    pub const INPUT_CAPTURE: &str = "input-capture";
    pub const PROCESS: &str = "process";
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum JobExecutorKind {
    EngineWorker,
    MainThreadBarrier,
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
            Self::MainThreadBarrier => "main-thread-barrier",
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
    pub fn new(mut event: EngineTaskEvent, executor: JobExecutorKind, semantic_owner: impl Into<String>) -> Self {
        if event.executor.is_none() {
            event = event.with_executor(executor.as_str());
        }
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
    pub const RUN_PROCESS_START_V1: &str = "job.run_process_start_v1";
    pub const RESULT_BIN_V1: &str = "job.result_bin_v1";
    pub const INVOKE_SERVICE_V1: &str = "job.invoke_service_v1";
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
    jobs_method::RUN_PROCESS_START_V1,
    jobs_method::RESULT_BIN_V1,
    jobs_method::INVOKE_SERVICE_V1,
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
            provider: "ForgeJobsProvider".to_owned(),
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
    pub priority: String,
    pub frame_id: Option<u64>,
    pub dependency_group: String,
    pub job_pass: String,
    pub job_domain: String,
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
            priority: "normal".to_owned(),
            frame_id: None,
            dependency_group: String::new(),
            job_pass: "external".to_owned(),
            job_domain: "engine.jobs".to_owned(),
            can_pause: false,
            can_cancel: true,
        }
    }
}



#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct JobRunProcessStartRequestV1 {
    pub job_id: String,
    pub name: String,
    pub owner: String,
    pub category: String,
    pub lane: String,
    pub priority: String,
    pub executable: String,
    pub args: Vec<String>,
    pub cwd: String,
    pub env: BTreeMap<String, String>,
    pub result_path: String,
    pub frame_id: Option<u64>,
    pub dependency_group: String,
    pub job_pass: String,
    pub job_domain: String,
    pub can_cancel: bool,
}

impl Default for JobRunProcessStartRequestV1 {
    fn default() -> Self {
        Self {
            job_id: String::new(),
            name: "external-process".to_owned(),
            owner: "engine.jobs".to_owned(),
            category: "tool.process".to_owned(),
            lane: "render-prep".to_owned(),
            priority: "background".to_owned(),
            executable: String::new(),
            args: Vec::new(),
            cwd: String::new(),
            env: BTreeMap::new(),
            result_path: String::new(),
            frame_id: None,
            dependency_group: String::new(),
            job_pass: "process".to_owned(),
            job_domain: "engine.jobs".to_owned(),
            can_cancel: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct JobRunProcessStartedV1 {
    pub job_id: String,
    pub accepted: bool,
    pub status: String,
    pub detail: String,
    pub result_path: String,
}

impl Default for JobRunProcessStartedV1 {
    fn default() -> Self {
        Self {
            job_id: String::new(),
            accepted: false,
            status: "rejected".to_owned(),
            detail: String::new(),
            result_path: String::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct JobServiceCallTargetV1 {
    pub gateway: String,
    pub method: String,
    pub payload_json: serde_json::Value,
}

impl Default for JobServiceCallTargetV1 {
    fn default() -> Self {
        Self {
            gateway: String::new(),
            method: String::new(),
            payload_json: serde_json::Value::Null,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct JobServiceCallRequestV1 {
    pub job_id: String,
    pub name: String,
    pub owner: String,
    pub category: String,
    pub lane: String,
    pub priority: String,
    pub can_pause: bool,
    pub frame_id: Option<u64>,
    pub dependency_group: String,
    pub job_pass: String,
    pub job_domain: String,
    pub can_cancel: bool,
    pub target: JobServiceCallTargetV1,
}

impl Default for JobServiceCallRequestV1 {
    fn default() -> Self {
        Self {
            job_id: String::new(),
            name: "service-call-job".to_owned(),
            owner: "engine.jobs".to_owned(),
            category: "service-call".to_owned(),
            lane: "plugin".to_owned(),
            priority: "background".to_owned(),
            can_pause: false,
            frame_id: None,
            dependency_group: String::new(),
            job_pass: "service-call".to_owned(),
            job_domain: "engine.jobs".to_owned(),
            can_cancel: true,
            target: JobServiceCallTargetV1::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct JobServiceCallAcceptedV1 {
    pub job_id: String,
    pub accepted: bool,
    pub gateway: String,
    pub method: String,
    pub status: String,
    pub detail: String,
}

impl Default for JobServiceCallAcceptedV1 {
    fn default() -> Self {
        Self {
            job_id: String::new(),
            accepted: false,
            gateway: String::new(),
            method: String::new(),
            status: String::new(),
            detail: String::new(),
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
    pub frame_id: Option<u64>,
    pub dependency_group: String,
    pub job_pass: String,
    pub job_domain: String,
    pub priority: String,
    pub executor: String,
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
            frame_id: None,
            dependency_group: String::new(),
            job_pass: "runtime".to_owned(),
            job_domain: "engine.jobs".to_owned(),
            priority: "normal".to_owned(),
            executor: "external-provider".to_owned(),
            can_pause: false,
            can_cancel: true,
        }
    }
}

impl JobProgressEventV1 {
    pub fn into_task_event(self) -> EngineTaskEvent {
        let mut event = EngineTaskEvent::new(
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
        .with_job_domain(self.job_domain)
        .with_job_pass(self.job_pass)
        .with_priority(self.priority)
        .with_executor(self.executor);
        if let Some(frame_id) = self.frame_id {
            event = event.with_frame_id(frame_id);
        }
        if !self.dependency_group.trim().is_empty() {
            event = event.with_dependency_group(self.dependency_group);
        }
        event
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct JobStatusJsonV1 {
    pub job_id: String,
    pub name: String,
    pub lane: String,
    pub priority: String,
    pub frame_id: Option<u64>,
    pub dependency_group: String,
    pub job_pass: String,
    pub job_domain: String,
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
            frame_id: None,
            dependency_group: String::new(),
            job_pass: String::new(),
            job_domain: String::new(),
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
    #[serde(default)]
    pub lanes: BTreeMap<String, JobsLaneSnapshotJsonV1>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct JobsLaneSnapshotJsonV1 {
    pub pending_jobs: usize,
    pub running_jobs: usize,
    pub completed_jobs: u64,
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
            lanes: BTreeMap::new(),
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
