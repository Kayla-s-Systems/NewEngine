#![forbid(unsafe_op_in_unsafe_fn)]

use abi_stable::std_types::{RResult, RString, RVec};
use abi_stable::StableAbi;

use newengine_plugin_api::{
    CapabilityDesc, HostApiV1, PluginDescriptor, PluginInfo, PluginKind, PluginModule, PluginModuleV2,
};

use crate::logger::{init_console_logger, ConsoleLoggerConfig};

#[derive(Default, StableAbi)]
#[repr(C)]
pub struct LoggingPlugin {
    initialized: bool,
}

impl LoggingPlugin {
    fn init_impl(&mut self) -> Result<(), String> {
        if self.initialized {
            return Ok(());
        }

        let cfg = ConsoleLoggerConfig::from_env();
        init_console_logger(&cfg)?;

        // After init, log crate is ready.
        log::info!(
            "logging: initialized (file={:?} tee={} level={:?} filter={:?})",
            cfg.file_path,
            cfg.tee,
            cfg.level,
            cfg.filter
        );

        self.initialized = true;
        Ok(())
    }

    fn descriptor_impl(&self) -> PluginDescriptor {
        PluginDescriptor {
            id: RString::from("newengine.logging"),
            name: RString::from("NewEngine Logging"),
            version: RString::from(env!("CARGO_PKG_VERSION")),
            kind: PluginKind::Runtime,
            capabilities: RVec::<CapabilityDesc>::new(),
        }
    }

    fn info_impl(&self) -> PluginInfo {
        let d = self.descriptor_impl();
        PluginInfo {
            id: d.id,
            name: d.name,
            version: d.version,
        }
    }
}

impl PluginModuleV2 for LoggingPlugin {
    fn descriptor(&self) -> PluginDescriptor {
        self.descriptor_impl()
    }

    fn init(&mut self, _host: HostApiV1) -> RResult<(), RString> {
        match self.init_impl() {
            Ok(()) => RResult::ROk(()),
            Err(e) => RResult::RErr(RString::from(e)),
        }
    }

    fn start(&mut self) -> RResult<(), RString> {
        RResult::ROk(())
    }

    fn fixed_update(&mut self, _dt: f32) -> RResult<(), RString> {
        RResult::ROk(())
    }

    fn update(&mut self, _dt: f32) -> RResult<(), RString> {
        RResult::ROk(())
    }

    fn render(&mut self, _dt: f32) -> RResult<(), RString> {
        RResult::ROk(())
    }

    fn shutdown(&mut self) {
        // No-op: global logger lifetime is process-wide.
    }
}

impl PluginModule for LoggingPlugin {
    fn info(&self) -> PluginInfo {
        self.info_impl()
    }

    fn init(&mut self, host: HostApiV1) -> RResult<(), RString> {
        <Self as PluginModuleV2>::init(self, host)
    }

    fn start(&mut self) -> RResult<(), RString> {
        <Self as PluginModuleV2>::start(self)
    }

    fn fixed_update(&mut self, dt: f32) -> RResult<(), RString> {
        <Self as PluginModuleV2>::fixed_update(self, dt)
    }

    fn update(&mut self, dt: f32) -> RResult<(), RString> {
        <Self as PluginModuleV2>::update(self, dt)
    }

    fn render(&mut self, dt: f32) -> RResult<(), RString> {
        <Self as PluginModuleV2>::render(self, dt)
    }

    fn shutdown(&mut self) {
        <Self as PluginModuleV2>::shutdown(self)
    }
}
