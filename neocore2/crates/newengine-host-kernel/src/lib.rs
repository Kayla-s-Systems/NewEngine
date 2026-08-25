#![forbid(unsafe_op_in_unsafe_fn)]

use crossbeam_channel::unbounded;
use newengine_core::{
    Bus, Engine, EngineConfig, EngineResult, ServiceRegistry, Services, ShutdownToken,
};

/// Minimal service container owned by the host kernel.
///
/// Domain implementations are intentionally absent. Providers register through
/// the plugin host / gateway registry after composition begins.
struct KernelServices {
    registry: ServiceRegistry,
}

impl KernelServices {
    fn new() -> Self {
        Self {
            registry: ServiceRegistry::new(),
        }
    }
}

impl Services for KernelServices {
    fn service_registry(&self) -> &ServiceRegistry {
        &self.registry
    }
}

/// Constructs the host kernel from an already-resolved generic engine config.
/// No domain service, null backend or gameplay implementation is installed.
pub fn build_kernel_engine(config: EngineConfig) -> EngineResult<Engine<()>> {
    build_kernel_engine_with_registry(config, |_| {})
}

/// Constructs a kernel Engine inside an already-created host universe. This is
/// used by editor/PIE/preview orchestration that performs preinit before Engine
/// construction.
pub fn build_kernel_engine_with_host(
    config: EngineConfig,
    host: newengine_plugin_host::HostContextHandle,
) -> EngineResult<Engine<()>> {
    build_kernel_engine_with_registry_and_host(config, host, |_| {})
}

/// Constructs the kernel while allowing an upper composition layer to register
/// host-local service adapters before the engine takes ownership of the registry.
/// The kernel itself still does not know which domains those adapters implement.
pub fn build_kernel_engine_with_registry<F>(
    config: EngineConfig,
    configure: F,
) -> EngineResult<Engine<()>>
where
    F: FnOnce(&ServiceRegistry),
{
    let host = newengine_plugin_host::create_host_context();
    build_kernel_engine_with_registry_and_host(config, host, configure)
}

pub fn build_kernel_engine_with_registry_and_host<F>(
    config: EngineConfig,
    host: newengine_plugin_host::HostContextHandle,
    configure: F,
) -> EngineResult<Engine<()>>
where
    F: FnOnce(&ServiceRegistry),
{
    newengine_plugin_host::activate_host_context(&host);
    let (tx, rx) = unbounded::<()>();
    let bus = Bus::new(tx, rx);
    let services = KernelServices::new();
    configure(services.service_registry());
    let services: Box<dyn Services> = Box::new(services);
    Engine::new_with_config_and_host(config, services, bus, ShutdownToken::new(), host)
}

/// Smallest runnable NewEngine host assembly.
///
/// It can load plugins if the caller later adds discovery roots, but contains no
/// render/physics/assets/UI/input/world/gameplay requirement by itself.
pub fn build_empty_host(fixed_dt_ms: u32) -> EngineResult<Engine<()>> {
    build_kernel_engine(EngineConfig::new(fixed_dt_ms).with_implicit_plugin_discovery(false))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_host_constructs_without_domain_features() {
        let engine = build_empty_host(16).expect("empty host kernel");
        assert_eq!(engine.run_state(), newengine_core::EngineRunState::Created);
        assert!(
            !newengine_plugin_host::has_service("engine.command"),
            "empty Void Host must not construct the optional command console provider"
        );
    }

    #[test]
    fn empty_host_reaches_running_without_domain_providers() {
        let mut engine = build_empty_host(16).expect("empty host kernel");
        engine.start().expect("empty host startup");
        assert_eq!(engine.run_state(), newengine_core::EngineRunState::Running);
        engine.shutdown().expect("empty host shutdown");
        assert_eq!(engine.run_state(), newengine_core::EngineRunState::Stopped);
    }
}
