#![forbid(unsafe_op_in_unsafe_fn)]

use crossbeam_channel::unbounded;
use newengine_core::{
    Bus, Engine, EngineConfig, EngineResult, ModuleFaultTolerance, PluginFaultTolerance,
    Services, ShutdownToken, StartupConfig,
};
use newengine_ui::UiProviderKind;

struct DefaultHostServices {
    registry: newengine_core::ServiceRegistry,
}

impl DefaultHostServices {
    #[inline]
    fn new() -> Self {
        Self {
            registry: newengine_core::ServiceRegistry::new(),
        }
    }
}

impl Services for DefaultHostServices {
    #[inline]
    fn logger(&self) -> &dyn log::Log {
        log::logger()
    }

    #[inline]
    fn service_registry(&self) -> &newengine_core::ServiceRegistry {
        &self.registry
    }
}

#[inline]
pub fn ui_provider_kind_from_startup(startup: &StartupConfig) -> UiProviderKind {
    match startup.ui_backend.plugin_id() {
        Some(service_id) => UiProviderKind::Plugin {
            service_id: service_id.to_owned(),
        },
        None => UiProviderKind::Null,
    }
}

pub fn build_engine_from_startup(
    startup: &StartupConfig,
    fixed_dt_ms: u32,
) -> EngineResult<Engine<()>> {
    let (tx, rx) = unbounded::<()>();
    let bus: Bus<()> = Bus::new(tx, rx);

    let host_services = DefaultHostServices::new();
    newengine_transform::service::register(host_services.service_registry());
    let services: Box<dyn Services> = Box::new(host_services);
    let shutdown = ShutdownToken::new();

    let config = EngineConfig::new(fixed_dt_ms)
        .with_plugins_dir(Some(startup.modules_dir.clone()))
        .with_module_fault_tolerance(ModuleFaultTolerance::Strict)
        .with_plugin_fault_tolerance(PluginFaultTolerance::Strict);

    Engine::new_with_config(config, services, bus, shutdown)
}
