#![forbid(unsafe_op_in_unsafe_fn)]

use abi_stable::std_types::RResult;

use newengine_plugin_api::{PluginModuleDyn, PluginModuleV2Dyn, PluginModuleV3Dyn};

/// Unified runtime ops surface for all ABI versions used by the host loop.
pub(crate) trait ModuleOps {
    fn start(&mut self) -> RResult<(), abi_stable::std_types::RString>;
    fn fixed_update(&mut self, dt: f32) -> RResult<(), abi_stable::std_types::RString>;
    fn update(&mut self, dt: f32) -> RResult<(), abi_stable::std_types::RString>;
    fn render(&mut self, dt: f32) -> RResult<(), abi_stable::std_types::RString>;
    fn shutdown(&mut self);
}

/// V1 adapter.
pub(crate) struct V1Adapter {
    pub(crate) module: PluginModuleDyn<'static>,
}

impl ModuleOps for V1Adapter {
    #[inline]
    fn start(&mut self) -> RResult<(), abi_stable::std_types::RString> {
        self.module.start()
    }

    #[inline]
    fn fixed_update(&mut self, dt: f32) -> RResult<(), abi_stable::std_types::RString> {
        self.module.fixed_update(dt)
    }

    #[inline]
    fn update(&mut self, dt: f32) -> RResult<(), abi_stable::std_types::RString> {
        self.module.update(dt)
    }

    #[inline]
    fn render(&mut self, dt: f32) -> RResult<(), abi_stable::std_types::RString> {
        self.module.render(dt)
    }

    #[inline]
    fn shutdown(&mut self) {
        self.module.shutdown();
    }
}

/// V2 adapter.
pub(crate) struct V2Adapter {
    pub(crate) module: PluginModuleV2Dyn<'static>,
}

impl ModuleOps for V2Adapter {
    #[inline]
    fn start(&mut self) -> RResult<(), abi_stable::std_types::RString> {
        self.module.start()
    }

    #[inline]
    fn fixed_update(&mut self, dt: f32) -> RResult<(), abi_stable::std_types::RString> {
        self.module.fixed_update(dt)
    }

    #[inline]
    fn update(&mut self, dt: f32) -> RResult<(), abi_stable::std_types::RString> {
        self.module.update(dt)
    }

    #[inline]
    fn render(&mut self, dt: f32) -> RResult<(), abi_stable::std_types::RString> {
        self.module.render(dt)
    }

    #[inline]
    fn shutdown(&mut self) {
        self.module.shutdown();
    }
}

/// V3 adapter.
pub(crate) struct V3Adapter {
    pub(crate) module: PluginModuleV3Dyn<'static>,
}

impl ModuleOps for V3Adapter {
    #[inline]
    fn start(&mut self) -> RResult<(), abi_stable::std_types::RString> {
        self.module.start()
    }

    #[inline]
    fn fixed_update(&mut self, dt: f32) -> RResult<(), abi_stable::std_types::RString> {
        self.module.fixed_update(dt)
    }

    #[inline]
    fn update(&mut self, dt: f32) -> RResult<(), abi_stable::std_types::RString> {
        self.module.update(dt)
    }

    #[inline]
    fn render(&mut self, dt: f32) -> RResult<(), abi_stable::std_types::RString> {
        self.module.render(dt)
    }

    #[inline]
    fn shutdown(&mut self) {
        self.module.shutdown();
    }
}

/// ABI dispatch container.
/// This keeps "which version?" logic out of the game loop.
pub(crate) enum ModuleAdapterAny {
    V1(V1Adapter),
    V2(V2Adapter),
    V3(V3Adapter),
}

impl ModuleAdapterAny {
    #[inline]
    pub(crate) fn as_v1(&self) -> Option<&PluginModuleDyn<'static>> {
        match self {
            Self::V1(a) => Some(&a.module),
            _ => None,
        }
    }

    #[inline]
    pub(crate) fn start(&mut self) -> RResult<(), abi_stable::std_types::RString> {
        match self {
            Self::V1(a) => a.start(),
            Self::V2(a) => a.start(),
            Self::V3(a) => a.start(),
        }
    }

    #[inline]
    pub(crate) fn fixed_update(&mut self, dt: f32) -> RResult<(), abi_stable::std_types::RString> {
        match self {
            Self::V1(a) => a.fixed_update(dt),
            Self::V2(a) => a.fixed_update(dt),
            Self::V3(a) => a.fixed_update(dt),
        }
    }

    #[inline]
    pub(crate) fn update(&mut self, dt: f32) -> RResult<(), abi_stable::std_types::RString> {
        match self {
            Self::V1(a) => a.update(dt),
            Self::V2(a) => a.update(dt),
            Self::V3(a) => a.update(dt),
        }
    }

    #[inline]
    pub(crate) fn render(&mut self, dt: f32) -> RResult<(), abi_stable::std_types::RString> {
        match self {
            Self::V1(a) => a.render(dt),
            Self::V2(a) => a.render(dt),
            Self::V3(a) => a.render(dt),
        }
    }

    #[inline]
    pub(crate) fn shutdown(&mut self) {
        match self {
            Self::V1(a) => a.shutdown(),
            Self::V2(a) => a.shutdown(),
            Self::V3(a) => a.shutdown(),
        }
    }
}
