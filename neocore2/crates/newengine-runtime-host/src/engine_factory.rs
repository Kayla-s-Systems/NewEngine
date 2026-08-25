#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_core::{
    Engine, EngineConfig, EngineResult, ModuleFaultTolerance, PluginFaultTolerance, StartupConfig,
};
pub fn build_engine_from_startup(
    startup: &StartupConfig,
    fixed_dt_ms: u32,
) -> EngineResult<Engine<()>> {
    let host = newengine_plugin_host::create_host_context();
    build_engine_from_startup_with_host(startup, fixed_dt_ms, host)
}

pub fn build_engine_from_startup_with_host(
    startup: &StartupConfig,
    fixed_dt_ms: u32,
    host: newengine_plugin_host::HostContextHandle,
) -> EngineResult<Engine<()>> {
    let config = EngineConfig::new(fixed_dt_ms)
        .with_plugins_dir(Some(startup.modules_dir.clone()))
        .with_plugin_overrides(startup.plugins.clone())
        .with_module_fault_tolerance(ModuleFaultTolerance::Strict)
        .with_plugin_fault_tolerance(PluginFaultTolerance::Strict);

    #[cfg(feature = "full-runtime")]
    let engine = newengine_host_kernel::build_kernel_engine_with_registry_and_host(
        config,
        host.clone(),
        |registry| {
            // Transform is an upper runtime composition service, not part of the kernel.
            newengine_transform::service::register(registry);
        },
    )?;

    #[cfg(not(feature = "full-runtime"))]
    let engine = newengine_host_kernel::build_kernel_engine_with_host(config, host.clone())?;

    #[cfg(feature = "command-console")]
    let _ = newengine_console_runtime::install_console_provider();

    Ok(engine)
}

#[cfg(all(test, feature = "full-runtime"))]
mod tests {
    use super::*;

    #[test]
    fn engine_command_service_discovers_runtime_session_commands() {
        let startup = StartupConfig::default();
        let _engine = build_engine_from_startup(&startup, 16).expect("engine");
        let console_install = newengine_console_runtime::install_console_provider();
        eprintln!(
            "console_install={console_install} services={:?} route={:?}",
            newengine_core::list_service_ids(),
            newengine_core::resolve_service_for_engine_gateway(
                newengine_console_runtime::ENGINE_COMMAND_GATEWAY_ID
            )
        );
        newengine_runtime_session_runtime::init_runtime_session_command_service();
        eprintln!(
            "after_session services={:?} route={:?}",
            newengine_core::list_service_ids(),
            newengine_core::resolve_service_for_engine_gateway(
                newengine_console_runtime::ENGINE_COMMAND_GATEWAY_ID
            )
        );

        let description =
            newengine_core::describe_service(newengine_console_runtime::ENGINE_COMMAND_GATEWAY_ID)
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
