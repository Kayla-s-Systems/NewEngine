#![forbid(unsafe_op_in_unsafe_fn)]

use crate::error::{EngineError, EngineResult};
use newengine_plugin_host::PluginSnapshotEntry;

#[derive(Debug, Clone, Copy)]
struct RequiredRuntimeServiceContract {
    service_id: &'static str,
    expected_contract: &'static str,
    required_methods: &'static [&'static str],
    required_capability_id: Option<&'static str>,
    required_env: &'static str,
}

impl RequiredRuntimeServiceContract {
    #[inline]
    fn is_required(self) -> bool {
        env_flag(self.required_env)
    }
}

const PLATFORM_REQUIRED_METHODS: &[&str] = &[
    newengine_platform_api::PLATFORM_WINDOW_SERVICE_METHOD_SNAPSHOT_JSON_V1,
];

const CONTRACTS: &[RequiredRuntimeServiceContract] = &[
    RequiredRuntimeServiceContract {
        service_id: newengine_assets_api::ASSET_RUNTIME_CONTRACT_SPEC.service_id,
        expected_contract: newengine_assets_api::ASSET_RUNTIME_CONTRACT_SPEC.expected_contract,
        required_methods: newengine_assets_api::ASSET_RUNTIME_CONTRACT_SPEC.required_methods,
        required_capability_id: Some(newengine_assets_api::ASSET_BACKEND_CAPABILITY_ID),
        required_env: "NEWENGINE_REQUIRE_ASSET_MANAGER",
    },
    RequiredRuntimeServiceContract {
        service_id: newengine_render_api::ENGINE_RENDER_SERVICE_ID,
        expected_contract: "newengine.render-api >= 0.3.x",
        required_methods: newengine_service_api::JSON_CONTROL_SERVICE_METHODS_V1,
        required_capability_id: Some(newengine_render_api::RENDER_BACKEND_CAPABILITY_ID),
        required_env: "NEWENGINE_REQUIRE_RENDER_BACKEND",
    },
    RequiredRuntimeServiceContract {
        service_id: newengine_physics_api::ENGINE_PHYSICS_SERVICE_ID,
        expected_contract: "newengine.physics-api >= 0.1.x",
        required_methods: newengine_service_api::JSON_CONTROL_SERVICE_METHODS_V1,
        required_capability_id: Some(newengine_physics_api::PHYSICS_BACKEND_CAPABILITY_ID),
        required_env: "NEWENGINE_REQUIRE_PHYSICS_BACKEND",
    },
    RequiredRuntimeServiceContract {
        service_id: newengine_platform_api::PLATFORM_WINDOW_SERVICE_ID,
        expected_contract: "newengine.platform-api >= 0.1.x",
        required_methods: PLATFORM_REQUIRED_METHODS,
        required_capability_id: None,
        required_env: "NEWENGINE_REQUIRE_PLATFORM_WINDOW_SERVICE",
    },
];

pub(crate) fn validate_runtime_service_contracts(
    plugins: &[PluginSnapshotEntry],
) -> EngineResult<()> {
    for contract in CONTRACTS {
        validate_one(*contract, plugins)?;
    }
    Ok(())
}

fn validate_one(
    contract: RequiredRuntimeServiceContract,
    plugins: &[PluginSnapshotEntry],
) -> EngineResult<()> {
    let present = newengine_plugin_host::has_service(contract.service_id);
    let required = contract.is_required();

    if !present {
        return contract_violation(
            contract,
            plugins,
            format!("service '{}' is not registered", contract.service_id),
        );
    }

    let Some(description) = newengine_plugin_host::describe_service(contract.service_id) else {
        return contract_violation(
            contract,
            plugins,
            format!("service '{}' has no describe() contract", contract.service_id),
        );
    };

    let methods = match parse_methods_from_description(&description) {
        Ok(methods) => methods,
        Err(e) => {
            return contract_violation(
                contract,
                plugins,
                format!(
                    "service '{}' returned invalid describe() JSON: {e}",
                    contract.service_id
                ),
            );
        }
    };

    let mut missing = Vec::new();
    for required_method in contract.required_methods {
        if !methods.iter().any(|m| m == required_method) {
            missing.push(*required_method);
        }
    }

    if !missing.is_empty() {
        return contract_violation(
            contract,
            plugins,
            format!("missing method(s): {}", missing.join(", ")),
        );
    }

    if let Some(capability_id) = contract.required_capability_id {
        if !provider_with_service_and_capability_exists(plugins, contract.service_id, capability_id) {
            return contract_violation(
                contract,
                plugins,
                format!(
                    "provider must declare service '{}' and backend capability '{}'",
                    contract.service_id, capability_id
                ),
            );
        }
    }

    log::info!(
        "runtime contract ok: service='{}' expected='{}' required=[{}]",
        contract.service_id,
        contract.expected_contract,
        method_statuses(contract.required_methods).join(" ")
    );
    Ok(())
}


fn method_statuses(methods: &[&str]) -> Vec<String> {
    methods
        .iter()
        .map(|method| {
            let label = method
                .rsplit_once('.')
                .map(|(_, tail)| tail)
                .unwrap_or(method);
            format!("{label}=yes")
        })
        .collect()
}

fn parse_methods_from_description(description: &str) -> Result<Vec<String>, String> {
    let v: serde_json::Value = serde_json::from_str(description).map_err(|e| e.to_string())?;
    let Some(methods) = v.get("methods").and_then(|x| x.as_array()) else {
        return Err("missing methods[]".to_owned());
    };

    let mut out = Vec::with_capacity(methods.len());
    for item in methods {
        if let Some(name) = item.as_str() {
            out.push(name.to_owned());
            continue;
        }
        if let Some(name) = item.get("name").and_then(|x| x.as_str()) {
            out.push(name.to_owned());
        }
    }
    out.sort();
    out.dedup();
    Ok(out)
}

fn contract_violation(
    contract: RequiredRuntimeServiceContract,
    plugins: &[PluginSnapshotEntry],
    reason: String,
) -> EngineResult<()> {
    if contract.is_required() {
        return Err(contract_error(contract, plugins, reason));
    }

    let provider = provider_for(plugins, contract.service_id);
    log::warn!(
        "runtime contract degraded: service='{}' provider='{}' expected='{}' reason='{}'",
        contract.service_id,
        provider,
        contract.expected_contract,
        reason
    );
    Ok(())
}

fn contract_error(
    contract: RequiredRuntimeServiceContract,
    plugins: &[PluginSnapshotEntry],
    reason: String,
) -> EngineError {
    let provider = provider_for(plugins, contract.service_id);
    EngineError::Other(format!(
        "FATAL: runtime service contract mismatch. service='{}' provider='{}' expected='{}' {}; rebuild/copy the matching plugin DLL before scene bootstrap.",
        contract.service_id,
        provider,
        contract.expected_contract,
        reason,
    ))
}

fn provider_with_service_and_capability_exists(
    plugins: &[PluginSnapshotEntry],
    service_id: &str,
    capability_id: &str,
) -> bool {
    if newengine_plugin_host::resolve_service_for_engine_gateway(service_id).is_some() {
        return true;
    }

    plugins.iter().any(|plugin| {
        let has_service = plugin.capabilities.iter().any(|cap| {
            cap.role == newengine_plugin_api::CapabilityRole::Provides
                && cap.kind == newengine_plugin_api::CapabilityKind::ServiceV1
                && cap.id.as_str() == service_id
        });
        let has_capability = plugin.capabilities.iter().any(|cap| {
            cap.role == newengine_plugin_api::CapabilityRole::Provides
                && cap.id.as_str() == capability_id
        });
        has_service && has_capability
    })
}

fn capability_engine_gateway(capability: &newengine_plugin_api::CapabilityDesc) -> Option<String> {
    if capability.role != newengine_plugin_api::CapabilityRole::Provides {
        return None;
    }
    serde_json::from_str::<serde_json::Value>(capability.describe_json.as_str())
        .ok()
        .and_then(|value| value.get("engine_gateway").and_then(|v| v.as_str()).map(str::to_owned))
}


fn provider_for(plugins: &[PluginSnapshotEntry], service_id: &str) -> String {
    let mut providers = plugins
        .iter()
        .filter(|plugin| {
            let declares_gateway = plugin.capabilities.iter().any(|cap| {
                capability_engine_gateway(cap)
                    .as_deref()
                    .is_some_and(|gateway| gateway == service_id)
            });
            let declares_service = plugin.capabilities.iter().any(|cap| {
                cap.role == newengine_plugin_api::CapabilityRole::Provides
                    && cap.kind == newengine_plugin_api::CapabilityKind::ServiceV1
                    && cap.id.as_str() == service_id
            });
            declares_gateway || declares_service
        })
        .map(|plugin| {
            format!(
                "{}@{} state={} path={}",
                plugin.id,
                plugin.version,
                plugin.state,
                plugin.path.display()
            )
        })
        .collect::<Vec<_>>();

    providers.sort();
    if providers.is_empty() {
        "<unknown>".to_owned()
    } else {
        providers.join("; ")
    }
}

fn env_flag(name: &str) -> bool {
    matches!(
        std::env::var(name).ok().as_deref(),
        Some("1") | Some("true") | Some("TRUE") | Some("yes") | Some("YES")
    )
}
