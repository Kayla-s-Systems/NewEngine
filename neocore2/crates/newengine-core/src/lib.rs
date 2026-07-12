pub mod bus;
pub mod cache_files;
pub mod config_root;
pub mod console;
pub mod core_invariants;
pub mod crash;
pub mod engine;
pub mod error;
pub mod error_reporter;
pub mod events;
pub mod frame;
pub mod host_events;
pub mod host_services;
pub mod lifecycle_events;
pub mod loading;
pub(crate) mod log_fmt;
pub mod module;
pub(crate) mod path_fmt;
pub(crate) mod plugin_forward_logger;
pub mod render;
pub mod storage_root;
mod task_core;
pub mod audio {
    pub use newengine_audio_api::*;
}
pub mod camera {
    pub use newengine_camera_api::*;
}
pub mod time {
    pub use newengine_time_api::*;
}
pub mod ui {
    pub use newengine_ui_api::*;
}
pub mod physics;
pub mod run_id;
pub mod sched;
pub mod services_registry;
pub mod startup;
pub mod startup_status;
pub mod startup_window;
pub mod sync;
mod system_info;
pub mod threading;

pub use host_services::{
    call_service_v1, call_service_v1_optional, describe_service, engine_gateway_has_capability,
    has_engine_gateway_route, list_service_ids, resolve_service_for_engine_gateway,
};

pub use newengine_service_api::{InterfaceId, ServiceInterface, ServiceKey};
pub use services_registry::{ErasedService, MissingServicePolicy, ServiceRegistry};

pub use bus::Bus;
pub use cache_files::{
    cache_child, publish_cache_files_env, resolve_cache_files_dir, resolve_under_cache_root,
    CACHE_FILES_ALIAS_ENV, CACHE_FILES_ENV, CACHE_FILES_READY_ENV,
};
pub use config_root::{
    config_child, publish_config_env, resolve_config_dir, resolve_under_config_root,
    CONFIG_ALIAS_ENV, CONFIG_ENV, CONFIG_READY_ENV,
};
pub use engine::{
    Engine, EngineConfig, EngineFsm, EngineFsmTransition, EngineRunState, ModuleFaultTolerance,
    PluginFaultTolerance,
};
pub use error::{EngineError, EngineResult, ModuleStage};
pub use error_reporter::{EngineErrorReporter, EngineErrorReporterConfig};
pub use events::{EventHub, EventSub};
pub use frame::Frame;
pub use host_events::WindowHostEvent;
pub use lifecycle_events::{EngineLifecycleEvent, EngineReadinessKey, EngineReadinessSnapshot};
pub use module::{ApiProvide, ApiRequire, ApiVersion, Module, ModuleCtx, Resources, Services};
pub use sched::{
    ScheduleBudgetClass, SchedulePhase, SchedulePhaseStats, ScheduleRunReport, ScheduleTaskDesc,
    Scheduler, SchedulerSnapshot,
};
pub use startup_status::{
    EngineIncrementalStartupState, EngineStartupPhase, EngineStartupSnapshot,
    EngineStartupStepOutcome, EngineStartupStepPhase, EngineStartupSystemPhase,
    EngineStartupSystemStatus,
};
pub use sync::ShutdownToken;

pub use threading::{
    CpuTaskDto, CpuTaskPriority, CpuTaskResultDto, CpuTaskTicket, TaskContext, TaskRuntimeStatus,
    TaskStatus, TaskTicket, ThreadPoolConfig, ThreadPoolHandle, ThreadPoolLaneSnapshot,
    ThreadPoolManager, ThreadPoolSnapshot, ENGINE_THREADING_GATEWAY_ID,
    THREADING_BACKEND_CAPABILITY_ID, THREADING_PROVIDER_SERVICE_ID, THREADING_RUNTIME_CONTRACT,
};

pub use task_core::{
    TaskLane, TaskPriority, TaskRequest, DEFAULT_FRAME_CPU_BUDGET_MS, JOB_LANE_COUNT,
    JOB_PRIORITY_COUNT,
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
    StartupOverride, StartupResolvedFrom, StartupStorageRootKind, WindowPlacement,
};
pub use startup_window::{
    startup_launch_settings, GraphicsPreset, ShadowQuality, StartupDisplaySettings,
    StartupGraphicsSettings, StartupHdrMode, StartupLaunchSettings, StartupWindowDecision,
    StartupWindowMode, StartupWindowReport, StartupWindowSelection, TextureQuality,
    STARTUP_SETTINGS_SCHEMA_VERSION,
};
