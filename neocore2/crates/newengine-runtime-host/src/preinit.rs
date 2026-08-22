#![forbid(unsafe_op_in_unsafe_fn)]

use std::sync::Arc;

use newengine_host_capabilities_api::HostPreInitSnapshot;

pub fn run_host_preinit() -> Arc<HostPreInitSnapshot> {
    crate::host_early_log!("host.preinit.begin");
    let snapshot = Arc::new(newengine_host_capabilities_runtime::discover_preinit_snapshot());
    newengine_host_capabilities_runtime::emit_preinit_diagnostics(&snapshot);
    install_runtime_capability_policy(&snapshot);
    crate::host_early_log!(
        "host.preinit.ok logical_cores={} physical_cores={} gpu={} storage={} displays={} provider_hints={}",
        snapshot.capabilities.cpu.logical_cores.map(|value| value.to_string()).unwrap_or_else(|| "?".to_owned()),
        snapshot.capabilities.cpu.physical_cores.map(|value| value.to_string()).unwrap_or_else(|| "?".to_owned()),
        snapshot.capabilities.gpu.len(),
        snapshot.capabilities.storage.len(),
        snapshot.capabilities.displays.len(),
        snapshot.runtime_policy.provider_hints.len(),
    );
    snapshot
}

fn install_runtime_capability_policy(snapshot: &HostPreInitSnapshot) {
    newengine_plugin_host::clear_engine_gateway_selection_policies();
    for hint in &snapshot.runtime_policy.provider_hints {
        let policy = newengine_plugin_host::EngineGatewaySelectionPolicy::new(
            hint.gateway_id.clone(),
            "newengine-runtime-host.preinit",
        )
        .prefer_tags(hint.preferred_system_tags.clone())
        .forbid_tags(hint.forbidden_system_tags.clone())
        .preference_bonus(hint.preference_bonus);
        if let Err(error) = newengine_plugin_host::install_engine_gateway_selection_policy(policy) {
            newengine_ulog_api::ulog::warn!(
                "[NxHost] capability policy install failed gateway='{}' reason='{}' err='{}'",
                hint.gateway_id,
                hint.reason,
                error,
            );
        } else {
            newengine_ulog_api::ulog::info!(
                "[NxHost] CapabilityResolution gateway='{}' prefer='{}' forbid='{}' bonus={} reason='{}'",
                hint.gateway_id,
                hint.preferred_system_tags.join(","),
                hint.forbidden_system_tags.join(","),
                hint.preference_bonus,
                hint.reason,
            );
        }
    }
}
