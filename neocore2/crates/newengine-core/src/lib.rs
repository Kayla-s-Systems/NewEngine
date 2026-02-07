pub mod bus;
pub mod core_invariants;
pub mod engine;
pub mod error;
pub mod events;
pub mod frame;
pub mod host_events;
pub mod module;
pub mod plugins;
pub mod sched;
pub mod sync;
mod system_info;
pub mod render;
pub mod startup;
pub mod assets;
pub mod assets_service;
pub mod console;
pub mod host_services;

pub use host_services::{call_service_v1, describe_service, list_service_ids};

pub use assets::{AssetManager, AssetManagerConfig};

pub use bus::Bus;
pub use engine::{Engine, EngineConfig};
pub use error::{EngineError, EngineResult, ModuleStage};
pub use events::{EventHub, EventSub};
pub use frame::Frame;
pub use host_events::WindowHostEvent;
pub use module::{ApiProvide, ApiRequire, ApiVersion, Module, ModuleCtx, Resources, Services};
pub use sched::Scheduler;
pub use sync::ShutdownToken;

pub use render::{
    BeginFrameDesc, Color4, RenderApi, RenderApiRef, RENDER_API_ID, RENDER_API_PROVIDE,
    RENDER_API_VERSION,
};

pub use startup::{
    ConfigPaths,
    StartupConfig,
    StartupConfigSource,
    StartupLoadReport,
    StartupLoader,
    StartupOverride,
    StartupResolvedFrom,
    WindowPlacement,
};
