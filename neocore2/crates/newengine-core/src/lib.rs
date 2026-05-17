pub mod bus;
pub mod cache_files;
pub mod console;
pub mod core_invariants;
pub mod engine;
pub mod error;
pub mod crash;
pub mod error_reporter;
pub mod events;
pub mod frame;
pub mod host_events;
pub mod jobs;
pub mod lifecycle_events;
pub mod host_services;
pub mod module;
pub(crate) mod plugin_forward_logger;
pub(crate) mod log_fmt;
pub(crate) mod path_fmt;
pub mod render;
pub mod camera {
    pub use newengine_camera_api::*;
}
pub mod physics;
pub mod run_id;
pub mod sched;
pub mod services_registry;
pub mod startup;
pub mod startup_status;
pub mod sync;
mod system_info;

pub use host_services::{call_service_v1, call_service_v1_optional, describe_service, list_service_ids};

pub use newengine_service_api::{InterfaceId, ServiceInterface, ServiceKey};
pub use services_registry::{ErasedService, MissingServicePolicy, ServiceRegistry};

pub use bus::Bus;
pub use cache_files::{cache_child, publish_cache_files_env, resolve_cache_files_dir, resolve_under_cache_root, CACHE_FILES_ENV, CACHE_FILES_ENV_LEGACY, CACHE_FILES_READY_ENV};
pub use engine::{Engine, EngineConfig, EngineFsm, EngineFsmTransition, EngineRunState, ModuleFaultTolerance, PluginFaultTolerance};
pub use error::{EngineError, EngineResult, ModuleStage};
pub use error_reporter::{EngineErrorReporter, EngineErrorReporterConfig};
pub use events::{EventHub, EventSub};
pub use frame::Frame;
pub use host_events::WindowHostEvent;
pub use lifecycle_events::{EngineLifecycleEvent, EngineReadinessKey, EngineReadinessSnapshot};
pub use startup_status::{
    EngineIncrementalStartupState, EngineStartupPhase, EngineStartupSnapshot,
    EngineStartupStepOutcome, EngineStartupStepPhase, EngineStartupSystemPhase,
    EngineStartupSystemStatus,
};
pub use module::{ApiProvide, ApiRequire, ApiVersion, Module, ModuleCtx, Resources, Services};
pub use sched::{
    ScheduleBudgetClass, SchedulePhase, SchedulePhaseStats, ScheduleRunReport, ScheduleTaskDesc,
    Scheduler, SchedulerSnapshot,
};
pub use sync::ShutdownToken;

pub use jobs::{
    JobLane, JobPriority, JobRequest, JobSystem, JobSystemConfig, JobSystemHandle,
    JobSystemSnapshot, JobTicket, JOB_LANE_COUNT, JOB_PRIORITY_COUNT,
};

pub use run_id::{init_run_id, run_id};

pub use render::{
    BeginFrameDesc, BeginRenderTargetDesc, Color4, RenderApi, RenderApiRef, RenderTargetDesc,
    RenderTargetId, RENDER_API_ID, RENDER_API_PROVIDE, RENDER_API_VERSION,
};

pub use physics::{
    PhysicsApi, PhysicsApiRef, PHYSICS_API_ID, PHYSICS_API_PROVIDE, PHYSICS_API_VERSION,
};

pub use startup::{
    ConfigPaths, StartupConfig, StartupConfigSource, StartupLoadReport, StartupLoader,
    StartupOverride, StartupResolvedFrom, WindowPlacement,
};
