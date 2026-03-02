pub mod bus;
pub mod console;
pub mod core_invariants;
pub mod engine;
pub mod error;
pub mod crash;
pub mod error_reporter;
pub mod events;
pub mod frame;
pub mod host_events;
pub mod host_services;
pub mod module;
pub mod plugins;
pub mod render;
pub mod sched;
pub mod services_registry;
pub mod startup;
pub mod sync;
mod system_info;

pub use host_services::{call_service_v1, describe_service, list_service_ids};

pub use newengine_service_api::{InterfaceId, ServiceInterface, ServiceKey};
pub use services_registry::{ErasedService, MissingServicePolicy, ServiceRegistry};

pub use bus::Bus;
pub use engine::{Engine, EngineConfig};
pub use error::{EngineError, EngineResult, ModuleStage};
pub use error_reporter::{EngineErrorReporter, EngineErrorReporterConfig};
pub use events::{EventHub, EventSub};
pub use frame::Frame;
pub use host_events::WindowHostEvent;
pub use module::{ApiProvide, ApiRequire, ApiVersion, Module, ModuleCtx, Resources, Services};
pub use sched::Scheduler;
pub use sync::ShutdownToken;

pub use render::{
    BeginFrameDesc, BeginRenderTargetDesc, Color4, RenderApi, RenderApiRef, RenderTargetDesc,
    RenderTargetId, RENDER_API_ID, RENDER_API_PROVIDE, RENDER_API_VERSION,
};

pub use startup::{
    ConfigPaths, StartupConfig, StartupConfigSource, StartupLoadReport, StartupLoader,
    StartupOverride, StartupResolvedFrom, WindowPlacement,
};
