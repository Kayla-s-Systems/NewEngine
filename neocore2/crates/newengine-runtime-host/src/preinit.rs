#![forbid(unsafe_op_in_unsafe_fn)]

use std::sync::Arc;

use newengine_host_capabilities_api::{
    method, HostPreInitSnapshot, ENGINE_HOST_CAPABILITIES_GATEWAY_ID,
    HOST_CAPABILITIES_BACKEND_CAPABILITY_ID, HOST_CAPABILITIES_PROVIDER_ROUTE,
    HOST_CAPABILITIES_PROVIDER_SERVICE_ID, HOST_CAPABILITIES_SCHEMA_VERSION,
    HOST_CAPABILITIES_SERVICE_KIND,
};

pub fn run_host_preinit() -> Arc<HostPreInitSnapshot> {
    crate::host_early_log!("host.preinit.begin");

    // The gateway registry is the PreInit control plane as well. Profiles may
    // install a platform/test/remote provider before this call; the native
    // implementation is only the default provider when the route is absent.
    newengine_plugin_host::init_host_context();
    install_default_provider_if_missing();

    let snapshot = match query_snapshot() {
        Ok(snapshot) => snapshot,
        Err(error) => {
            newengine_ulog_api::ulog::warn!(
                "[NxHost] HostCapabilities unavailable gateway='{}' err='{}'; using neutral snapshot",
                ENGINE_HOST_CAPABILITIES_GATEWAY_ID,
                error,
            );
            HostPreInitSnapshot::default()
        }
    };
    let snapshot = Arc::new(snapshot);

    emit_preinit_diagnostics(&snapshot);
    install_runtime_capability_policy(&snapshot);
    crate::host_early_log!(
        "host.preinit.ok provider={} logical_cores={} physical_cores={} gpu={} storage={} displays={} provider_hints={}",
        newengine_core::resolve_service_for_engine_gateway(
            ENGINE_HOST_CAPABILITIES_GATEWAY_ID
        )
        .unwrap_or_else(|| "<none>".to_owned()),
        snapshot.capabilities.cpu.logical_cores.map(|value| value.to_string()).unwrap_or_else(|| "?".to_owned()),
        snapshot.capabilities.cpu.physical_cores.map(|value| value.to_string()).unwrap_or_else(|| "?".to_owned()),
        snapshot.capabilities.gpu.len(),
        snapshot.capabilities.storage.len(),
        snapshot.capabilities.displays.len(),
        snapshot.runtime_policy.provider_hints.len(),
    );
    snapshot
}

fn install_default_provider_if_missing() {
    if newengine_core::has_engine_gateway_route(ENGINE_HOST_CAPABILITIES_GATEWAY_ID) {
        return;
    }

    #[cfg(feature = "host-capabilities-native")]
    {
        let service = newengine_host_capabilities_runtime::native_host_capabilities_service();
        let registered =
            newengine_service_kit::register_engine_gateway_provider_service_dynamic_best_effort(
                newengine_service_kit::EngineGatewayProviderDeclDynamic {
                    gateway: ENGINE_HOST_CAPABILITIES_GATEWAY_ID,
                    service_kind: HOST_CAPABILITIES_SERVICE_KIND,
                    provider_service: HOST_CAPABILITIES_PROVIDER_SERVICE_ID,
                    provider_route: HOST_CAPABILITIES_PROVIDER_ROUTE,
                    capability: HOST_CAPABILITIES_BACKEND_CAPABILITY_ID,
                    priority: 0,
                    owner: "newengine-host-capabilities-runtime",
                    service,
                },
            );
        if !registered {
            newengine_ulog_api::ulog::warn!(
                "[NxHost] native HostCapabilities provider registration failed gateway='{}'",
                ENGINE_HOST_CAPABILITIES_GATEWAY_ID,
            );
        }
    }
}

fn query_snapshot() -> Result<HostPreInitSnapshot, String> {
    let bytes = newengine_core::call_service_v1(
        ENGINE_HOST_CAPABILITIES_GATEWAY_ID,
        method::SNAPSHOT,
        &[],
    )?;
    let snapshot: HostPreInitSnapshot = serde_json::from_slice(&bytes)
        .map_err(|error| format!("invalid HostPreInitSnapshot payload: {error}"))?;
    if snapshot.schema_version != HOST_CAPABILITIES_SCHEMA_VERSION {
        return Err(format!(
            "unsupported HostPreInitSnapshot schema={} expected={}",
            snapshot.schema_version, HOST_CAPABILITIES_SCHEMA_VERSION
        ));
    }
    Ok(snapshot)
}

fn emit_preinit_diagnostics(snapshot: &HostPreInitSnapshot) {
    let cpu = &snapshot.capabilities.cpu;
    newengine_ulog_api::ulog::info!(
        "[NxHost] PreInit os='{}' arch='{}' pid={} physical_cores={} logical_cores={} affinity='{:?}' avx={} f16c={} avx2={}",
        snapshot.environment.os,
        snapshot.environment.arch,
        snapshot.environment.pid,
        opt_u32(cpu.physical_cores),
        opt_u32(cpu.logical_cores),
        cpu.affinity_policy,
        cpu.features.avx as u8,
        cpu.features.f16c as u8,
        cpu.features.avx2 as u8,
    );
    newengine_ulog_api::ulog::info!(
        "[NxHost] HardwareDiscovery gpu={} storage={} displays={} keyboard={} mouse={} preferred_gpu='{}'",
        snapshot.capabilities.gpu.len(),
        snapshot.capabilities.storage.len(),
        snapshot.capabilities.displays.len(),
        opt_bool(snapshot.capabilities.input.keyboard_present),
        opt_bool(snapshot.capabilities.input.mouse_present),
        snapshot
            .capabilities
            .preferred_gpu()
            .map(|gpu| gpu.stable_id.as_str())
            .unwrap_or("<none>"),
    );
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

fn opt_u32(value: Option<u32>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "<unknown>".to_owned())
}

fn opt_bool(value: Option<bool>) -> &'static str {
    match value {
        Some(true) => "1",
        Some(false) => "0",
        None => "?",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn neutral_snapshot_is_versioned() {
        assert_eq!(
            HostPreInitSnapshot::default().schema_version,
            HOST_CAPABILITIES_SCHEMA_VERSION
        );
    }
}
