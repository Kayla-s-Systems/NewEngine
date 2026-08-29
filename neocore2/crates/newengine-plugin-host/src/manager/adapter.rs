#![forbid(unsafe_op_in_unsafe_fn)]

use abi_stable::std_types::{RResult, RString};
use newengine_plugin_api::PluginModuleDyn;

/// Unified runtime ops surface for the canonical plugin ABI.
pub(crate) struct ModuleAdapterAny {
    pub(crate) module: PluginModuleDyn<'static>,
}

impl ModuleAdapterAny {
    #[inline]
    pub(crate) fn new(module: PluginModuleDyn<'static>) -> Self {
        Self { module }
    }

    #[inline]
    pub(crate) fn module_ref(&self) -> &PluginModuleDyn<'static> {
        &self.module
    }

    #[inline]
    pub(crate) fn start(&mut self) -> RResult<(), RString> {
        self.module.start()
    }
    #[inline]
    pub(crate) fn fixed_update(&mut self, dt: f32) -> RResult<(), RString> {
        self.module.fixed_update(dt)
    }
    #[inline]
    pub(crate) fn update(&mut self, dt: f32) -> RResult<(), RString> {
        self.module.update(dt)
    }
    #[inline]
    pub(crate) fn render(&mut self, dt: f32) -> RResult<(), RString> {
        self.module.render(dt)
    }
    #[inline]
    pub(crate) fn shutdown(&mut self) {
        self.module.shutdown();
    }
}
