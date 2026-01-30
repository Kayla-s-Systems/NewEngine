#![forbid(unsafe_op_in_unsafe_fn)]
#![forbid(unsafe_op_in_unsafe_fn)]
#![allow(non_camel_case_types)]
#![allow(non_local_definitions)]

use abi_stable::library::RootModule;
use abi_stable::sabi_trait;
use abi_stable::sabi_types::VersionStrings;
use abi_stable::std_types::{RResult, RString};
use abi_stable::StableAbi;

#[derive(StableAbi, Clone)]
#[repr(C)]
pub struct PluginInfo {
    pub id: RString,
    pub version: RString,
}

#[derive(StableAbi, Clone)]
#[repr(C)]
pub struct HostApiV1 {
    pub log_info: extern "C" fn(msg: RString),
    pub log_warn: extern "C" fn(msg: RString),
    pub log_error: extern "C" fn(msg: RString),
    pub request_exit: extern "C" fn(),
    pub monotonic_time_ns: extern "C" fn() -> u64,
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

#[repr(C)]
#[derive(StableAbi)]
#[sabi(kind(Prefix(prefix_ref = PluginRootV1_Ref, prefix_fields = PluginRootV1_Prefix)))]
#[sabi(missing_field(panic))]
pub struct PluginRootV1 {
    #[sabi(last_prefix_field)]
    pub create: extern "C" fn() -> PluginModule_TO<'static, abi_stable::std_types::RBox<()>>,
}

impl RootModule for PluginRootV1_Ref {
    abi_stable::declare_root_module_statics! {PluginRootV1_Ref}

    const BASE_NAME: &'static str = "newengine_plugin";
    const NAME: &'static str = "newengine_plugin";
    const VERSION_STRINGS: VersionStrings = abi_stable::package_version_strings!();
}