#![forbid(unsafe_op_in_unsafe_fn)]

use crate::error::{EngineError, EngineResult};
use newengine_plugin_host::PluginSnapshotEntry;
use newengine_service_api::RuntimeServiceRequirementSpec;

use super::catalog::RUNTIME_SERVICE_REQUIREMENTS;
use super::description::{method_statuses, parse_methods_from_description};
use super::diagnostics::{provider_for, provider_has_required_capability};

pub(crate) fn validate_runtime_service_contracts(
    plugins: &[PluginSnapshotEntry],
) -> EngineResult<()> {
    for requirement in RUNTIME_SERVICE_REQUIREMENTS {
        validate_one(*requirement, plugins)?;
    }
    Ok(())
}

fn validate_one(
    requirement: RuntimeServiceRequirementSpec,
    plugins: &[PluginSnapshotEntry],
) -> EngineResult<()> {
    let contract = requirement.contract;
    let present = newengine_plugin_host::has_service(contract.service_id);

    if !present {
        return contract_violation(
            requirement,
            plugins,
            format!("service '{}' is not registered", contract.service_id),
        );
    }

    let Some(description) = newengine_plugin_host::describe_service(contract.service_id) else {
        return contract_violation(
            requirement,
            plugins,
            format!("service '{}' has no describe() contract", contract.service_id),
        );
    };

    let methods = match parse_methods_from_description(&description) {
        Ok(methods) => methods,
        Err(e) => {
            return contract_violation(
                requirement,
                plugins,
                format!(
                    "service '{}' returned invalid describe() JSON: {e}",
                    contract.service_id
                ),
            );
        }
    };

    let missing = contract
        .required_methods
        .iter()
        .copied()
        .filter(|required_method| !methods.iter().any(|method| method == required_method))
        .collect::<Vec<_>>();

    if !missing.is_empty() {
        return contract_violation(
            requirement,
            plugins,
            format!("missing method(s): {}", missing.join(", ")),
        );
    }

    if let Some(capability_id) = requirement.required_capability_id {
        if !provider_has_required_capability(contract.service_id, capability_id, plugins) {
            return contract_violation(
                requirement,
                plugins,
                format!(
                    "provider must route service '{}' through backend capability '{}'",
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

fn contract_violation(
    requirement: RuntimeServiceRequirementSpec,
    plugins: &[PluginSnapshotEntry],
    reason: String,
) -> EngineResult<()> {
    if is_required(requirement) {
        return Err(contract_error(requirement, plugins, reason));
    }

    let contract = requirement.contract;
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
    requirement: RuntimeServiceRequirementSpec,
    plugins: &[PluginSnapshotEntry],
    reason: String,
) -> EngineError {
    let contract = requirement.contract;
    let provider = provider_for(plugins, contract.service_id);
    EngineError::Other(format!(
        "FATAL: runtime service contract mismatch. service='{}' provider='{}' expected='{}' {}; rebuild/copy the matching plugin DLL before scene bootstrap.",
        contract.service_id,
        provider,
        contract.expected_contract,
        reason,
    ))
}

fn is_required(requirement: RuntimeServiceRequirementSpec) -> bool {
    requirement.required_env.is_some_and(env_flag)
}

fn env_flag(name: &str) -> bool {
    matches!(
        std::env::var(name).ok().as_deref(),
        Some("1") | Some("true") | Some("TRUE") | Some("yes") | Some("YES")
    )
}
