#![forbid(unsafe_op_in_unsafe_fn)]

use abi_stable::sabi_trait;
use abi_stable::std_types::{RResult, RString};
use abi_stable::StableAbi;

use crate::capability::PluginDescriptor;
use crate::host::HostApiV1;

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

#[sabi_trait]
pub trait PluginModuleV2: Send + Sync {
    fn descriptor(&self) -> PluginDescriptor;

    fn init(&mut self, host: HostApiV1) -> RResult<(), RString>;
    fn start(&mut self) -> RResult<(), RString>;

    fn fixed_update(&mut self, dt: f32) -> RResult<(), RString>;
    fn update(&mut self, dt: f32) -> RResult<(), RString>;
    fn render(&mut self, dt: f32) -> RResult<(), RString>;

    fn shutdown(&mut self);
}

pub type PluginModuleV2Dyn<'a> = PluginModuleV2_TO<'a, abi_stable::std_types::RBox<()>>;