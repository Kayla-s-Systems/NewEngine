#![forbid(unsafe_op_in_unsafe_fn)]
#![allow(non_local_definitions)]

use abi_stable::library::RootModule;
use abi_stable::sabi_trait;
use abi_stable::sabi_types::VersionStrings;
use abi_stable::std_types::{RResult, RString, RVec};
use abi_stable::StableAbi;

pub type Blob = RVec<u8>;
pub type CapabilityId = RString;
pub type MethodName = RString;

/* =============================================================================================
   Capability model (ABI-stable, extensible)
   ============================================================================================= */

/// Broad plugin category.
///
/// Values are ABI-stable; new kinds must be appended (do not reorder).
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, StableAbi)]
pub enum PluginKind {
    Runtime = 1,
    Importer = 2,
    Editor = 3,
    Tool = 4,

    /// Fallback for future extensions.
    Other = 255,
}

/// Capability direction.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, StableAbi)]
pub enum CapabilityRole {
    Provides = 1,
    Requires = 2,
}

/// Capability kind.
///
/// Keep this intentionally coarse: plugins own the semantics via `describe_json`.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, StableAbi)]
pub enum CapabilityKind {
    ServiceV1 = 1,
    EventsV1 = 2,

    /// Asset importer surface (still uses ServiceV1 call ABI).
    AssetImporterV1 = 3,

    Other = 255,
}

/// Small, ABI-stable capability descriptor.
///
/// `describe_json` is intentionally opaque to the core.
#[repr(C)]
#[derive(Debug, Clone, StableAbi)]
pub struct CapabilityDesc {
    pub id: CapabilityId,
    pub role: CapabilityRole,
    pub kind: CapabilityKind,
    pub version: u32,

    /// Provider-owned JSON (or empty string) describing the capability.
    pub describe_json: RString,
}

/// V2 plugin descriptor.
#[repr(C)]
#[derive(Debug, Clone, StableAbi)]
pub struct PluginDescriptor {
    pub id: RString,
    pub name: RString,
    pub version: RString,
    pub kind: PluginKind,
    pub capabilities: RVec<CapabilityDesc>,
}

/* =============================================================================================
   Generic service: semantics fully owned by provider plugin
   ============================================================================================= */

#[sabi_trait]
pub trait ServiceV1: Send + Sync {
    fn id(&self) -> CapabilityId;
    fn describe(&self) -> RString;
    fn call(&self, method: MethodName, payload: Blob) -> RResult<Blob, RString>;
}

pub type ServiceV1Dyn<'a> = ServiceV1_TO<'a, abi_stable::std_types::RBox<()>>;

#[sabi_trait]
pub trait EventSinkV1: Send + Sync {
    fn on_event(&mut self, topic: RString, payload: Blob);
}

pub type EventSinkV1Dyn<'a> = EventSinkV1_TO<'a, abi_stable::std_types::RBox<()>>;

/* =============================================================================================
   Host API: pure bridge
   ============================================================================================= */

#[repr(C)]
#[derive(Clone, StableAbi)]
pub struct HostApiV1 {
    pub log_info: extern "C" fn(RString),
    pub log_warn: extern "C" fn(RString),
    pub log_error: extern "C" fn(RString),

    pub register_service_v1: extern "C" fn(ServiceV1Dyn<'static>) -> RResult<(), RString>,

    /// Call an already registered service by id.
    /// This avoids returning service objects across ABI and avoids Clone requirements.
    pub call_service_v1: extern "C" fn(CapabilityId, MethodName, Blob) -> RResult<Blob, RString>,

    pub emit_event_v1: extern "C" fn(RString, Blob) -> RResult<(), RString>,
    pub subscribe_events_v1: extern "C" fn(EventSinkV1Dyn<'static>) -> RResult<(), RString>,
}

/* =============================================================================================
   Plugin module ABI (V1)
   ============================================================================================= */

#[repr(C)]
#[derive(Debug, Clone, StableAbi)]
pub struct PluginInfo {
    pub id: RString,
    pub name: RString,
    pub version: RString,
}

#[sabi_trait]
pub trait PluginModule: Send + Sync {
    fn info(&self) -> PluginInfo;

    fn init(&mut self, host: HostApiV1) -> RResult<(), RString>;
    fn start(&mut self) -> RResult<(), RString>;

    fn fixed_update(&mut self, dt: f32) -> RResult<(), RString>;
    fn update(&mut self, dt: f32) -> RResult<(), RString>;
    fn render(&mut self, dt: f32) -> RResult<(), RString>;

    fn shutdown(&mut self);
}

pub type PluginModuleDyn<'a> = PluginModule_TO<'a, abi_stable::std_types::RBox<()>>;

/* =============================================================================================
   Plugin module ABI (V2)
   ============================================================================================= */

#[sabi_trait]
pub trait PluginModuleV2: Send + Sync {
    /// Stronger, extensible descriptor with kind + capabilities.
    fn descriptor(&self) -> PluginDescriptor;

    fn init(&mut self, host: HostApiV1) -> RResult<(), RString>;
    fn start(&mut self) -> RResult<(), RString>;

    fn fixed_update(&mut self, dt: f32) -> RResult<(), RString>;
    fn update(&mut self, dt: f32) -> RResult<(), RString>;
    fn render(&mut self, dt: f32) -> RResult<(), RString>;

    fn shutdown(&mut self);
}

pub type PluginModuleV2Dyn<'a> = PluginModuleV2_TO<'a, abi_stable::std_types::RBox<()>>;

/* =============================================================================================
   Root module ABI
   ============================================================================================= */

#[repr(C)]
#[derive(StableAbi)]
#[sabi(kind(Prefix(prefix_ref = PluginRootV1Ref)))]
pub struct PluginRootV1 {
    /// Mark as the stable prefix boundary so older/newer binaries remain compatible
    /// when you add new optional fields later.
    #[sabi(last_prefix_field)]
    pub create: extern "C" fn() -> PluginModuleDyn<'static>,

    /// Optional V2 entrypoint.
    ///
    /// Old plugins won't have this field; hosts must treat it as optional.
    pub create_v2: extern "C" fn() -> PluginModuleV2Dyn<'static>,
}

impl RootModule for PluginRootV1Ref {
    abi_stable::declare_root_module_statics! { PluginRootV1Ref }

    const BASE_NAME: &'static str = "export_plugin_root";
    const NAME: &'static str = "export_plugin_root";
    const VERSION_STRINGS: VersionStrings = abi_stable::package_version_strings!();
}
