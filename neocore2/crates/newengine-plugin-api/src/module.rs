#![forbid(unsafe_op_in_unsafe_fn)]

use abi_stable::sabi_trait;
use abi_stable::std_types::{RBox, RResult, RString, RVec};
use abi_stable::StableAbi;

use crate::capability::PluginDescriptor;
use crate::config::{ConfigApplyResultV1, ConfigBlobV1, ConfigDiagV1, ConfigPatchV1};
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

pub type PluginModuleDyn<'a> = PluginModule_TO<'a, RBox<()>>;

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

pub type PluginModuleV2Dyn<'a> = PluginModuleV2_TO<'a, RBox<()>>;

/// V3 module contract with built-in configuration pipeline.
///
/// Host policy:
/// - host collects patches and sorts them by (priority ASC, name ASC)
/// - plugin validates/migrates and returns effective config
#[sabi_trait]
pub trait PluginModuleV3: Send + Sync {
    /// Plugin descriptor: id/kind/caps.
    fn descriptor_v3(&self) -> PluginDescriptor;

    // ---------------- Config contract (built-in) ----------------

    /// Returns plugin default configuration blob.
    fn config_defaults_v1(&self) -> RResult<ConfigBlobV1, RString>;

    /// Applies patches, migrations and validation; returns effective config and diagnostics.
    fn config_apply_patches_v1(
        &self,
        base: &ConfigBlobV1,
        patches: RVec<ConfigPatchV1>,
    ) -> RResult<ConfigApplyResultV1, RString>;

    /// Whether live config updates are supported after init.
    fn config_supports_live_update_v1(&self) -> bool;

    /// Applies effective config live after init.
    /// Should return diagnostics; use Error level if restart is required.
    fn config_update_live_v1(&mut self, effective: &ConfigBlobV1) -> RResult<RVec<ConfigDiagV1>, RString>;

    // ---------------- Lifecycle ----------------

    /// Initializes plugin with effective config.
    fn init_v3(&mut self, host: HostApiV1, effective: ConfigBlobV1) -> RResult<(), RString>;

    fn start(&mut self) -> RResult<(), RString>;

    fn fixed_update(&mut self, dt: f32) -> RResult<(), RString>;
    fn update(&mut self, dt: f32) -> RResult<(), RString>;
    fn render(&mut self, dt: f32) -> RResult<(), RString>;

    fn shutdown(&mut self);
}

pub type PluginModuleV3Dyn<'a> = PluginModuleV3_TO<'a, RBox<()>>;