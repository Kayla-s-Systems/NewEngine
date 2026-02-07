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
   Plugin module ABI
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
}

impl RootModule for PluginRootV1Ref {
    abi_stable::declare_root_module_statics! { PluginRootV1Ref }

    const BASE_NAME: &'static str = "export_plugin_root";
    const NAME: &'static str = "export_plugin_root";
    const VERSION_STRINGS: VersionStrings = abi_stable::package_version_strings!();
}