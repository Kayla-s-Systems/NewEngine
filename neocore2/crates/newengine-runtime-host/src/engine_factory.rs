#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_core::{
    Engine, EngineConfig, EngineResult, ModuleFaultTolerance, PluginFaultTolerance, StartupConfig,
};
#[cfg(feature = "full-runtime")]
use newengine_ui::UiProviderKind;

#[cfg(feature = "full-runtime")]
#[inline]
pub fn ui_provider_kind_from_startup(_startup: &StartupConfig) -> UiProviderKind {
    // UI provider selection is discovery-driven. Startup config must not bind
    // a concrete UI backend; the runtime host will bind the first registered
    // UI-provider service, or `none` when no provider exists.
    UiProviderKind::Null
}

pub fn build_engine_from_startup(
    startup: &StartupConfig,
    fixed_dt_ms: u32,
) -> EngineResult<Engine<()>> {
    let config = EngineConfig::new(fixed_dt_ms)
        .with_plugins_dir(Some(startup.modules_dir.clone()))
        .with_plugin_overrides(startup.plugins.clone())
        .with_module_fault_tolerance(ModuleFaultTolerance::Strict)
        .with_plugin_fault_tolerance(PluginFaultTolerance::Strict);

    #[cfg(feature = "full-runtime")]
    {
        return newengine_host_kernel::build_kernel_engine_with_registry(config, |registry| {
            // Transform is an upper runtime composition service, not part of the kernel.
            newengine_transform::service::register(registry);
        });
    }

    #[cfg(not(feature = "full-runtime"))]
    {
        newengine_host_kernel::build_kernel_engine(config)
    }
}

#[cfg(all(test, feature = "full-runtime"))]
mod tests {
    use super::*;

    #[test]
    fn engine_command_service_discovers_runtime_session_commands() {
        let startup = StartupConfig::default();
        let _engine = build_engine_from_startup(&startup, 16).expect("engine");
        newengine_runtime_session_runtime::init_runtime_session_command_service();

        let description =
            newengine_core::describe_service(newengine_core::console::COMMAND_SERVICE_ID)
                .expect("engine.command service");
        let value: serde_json::Value = serde_json::from_str(&description).expect("command json");
        let commands = value["console"]["commands"]
            .as_array()
            .expect("command descriptors");
        let ids = commands
            .iter()
            .filter_map(|command| command["id"].as_str())
            .collect::<std::collections::BTreeSet<_>>();

        assert!(ids.contains("runtime.play"));
        assert!(ids.contains("runtime.pause"));
        assert!(ids.contains("runtime.stop"));
        assert!(ids.contains("runtime.restart"));
        assert!(ids.contains("runtime.step"));
    }
}
