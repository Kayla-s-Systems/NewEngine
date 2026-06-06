#![forbid(unsafe_op_in_unsafe_fn)]

use crate::error::{EngineError, EngineResult};
use newengine_plugin_host::PluginSnapshotEntry;
use newengine_service_api::RuntimeServiceRequirementSpec;

use super::catalog::{runtime_service_user, RUNTIME_SERVICE_CATALOG};
use super::description::{method_statuses, parse_methods_from_description};
use super::diagnostics::{provider_for, provider_has_required_capability, service_origin};

#[derive(Debug, Clone)]
struct ContractReport {
    service_id: String,
    status: &'static str,
    provider_service: String,
    provider: String,
    source: String,
    capability: String,
    required: &'static str,
    used_by: &'static str,
    expected: String,
    methods: String,
    reason: String,
}

pub(crate) fn validate_runtime_service_contracts(
    plugins: &[PluginSnapshotEntry],
) -> EngineResult<()> {
    let duplicates = super::catalog::runtime_service_requirement_duplicates();
    if !duplicates.is_empty() {
        return Err(EngineError::Other(format!(
            "runtime service contract catalog contains duplicate API ids: {}",
            duplicates.join(", ")
        )));
    }

    let mut reports = Vec::with_capacity(RUNTIME_SERVICE_CATALOG.len());

    for entry in RUNTIME_SERVICE_CATALOG {
        let (report, error) = validate_one(entry.requirement, plugins);
        emit_contract_line(&report);
        reports.push(report);

        if let Some(error) = error {
            emit_runtime_api_table(&reports);
            return Err(error);
        }
    }

    emit_runtime_api_table(&reports);
    Ok(())
}

fn validate_one(
    requirement: RuntimeServiceRequirementSpec,
    plugins: &[PluginSnapshotEntry],
) -> (ContractReport, Option<EngineError>) {
    let contract = requirement.contract;
    let required = is_required(requirement);
    let provider = provider_for(plugins, contract.service_id);
    let provider_service = active_provider_service_id(contract.service_id);
    let source = active_route_source(contract.service_id);
    let capability = requirement
        .required_capability_id
        .unwrap_or("-")
        .to_owned();
    let used_by = runtime_service_user(contract.service_id);

    let mut report = ContractReport {
        service_id: contract.service_id.to_owned(),
        status: "ok",
        provider_service,
        provider,
        source,
        capability,
        required: if required { "yes" } else { "no" },
        used_by,
        expected: contract.expected_contract.to_owned(),
        methods: method_statuses(contract.required_methods).join(" "),
        reason: "-".to_owned(),
    };

    let present = newengine_plugin_host::has_service(contract.service_id);
    if !present {
        return finish_violation(
            requirement,
            plugins,
            report,
            format!("service '{}' is not registered", contract.service_id),
        );
    }

    let Some(description) = newengine_plugin_host::describe_service(contract.service_id) else {
        return finish_violation(
            requirement,
            plugins,
            report,
            format!("service '{}' has no describe() contract", contract.service_id),
        );
    };

    let methods = match parse_methods_from_description(&description) {
        Ok(methods) => methods,
        Err(e) => {
            return finish_violation(
                requirement,
                plugins,
                report,
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
        report.methods = contract
            .required_methods
            .iter()
            .map(|method| {
                let label = method.rsplit_once('.').map(|(_, tail)| tail).unwrap_or(method);
                if missing.iter().any(|missing_method| missing_method == method) {
                    format!("{label}=no")
                } else {
                    format!("{label}=yes")
                }
            })
            .collect::<Vec<_>>()
            .join(" ");

        return finish_violation(
            requirement,
            plugins,
            report,
            format!("missing method(s): {}", missing.join(", ")),
        );
    }

    if let Some(capability_id) = requirement.required_capability_id {
        if !provider_has_required_capability(contract.service_id, capability_id, plugins) {
            return finish_violation(
                requirement,
                plugins,
                report,
                format!(
                    "provider must route service '{}' through backend capability '{}'",
                    contract.service_id, capability_id
                ),
            );
        }
    }

    (report, None)
}

fn finish_violation(
    requirement: RuntimeServiceRequirementSpec,
    plugins: &[PluginSnapshotEntry],
    mut report: ContractReport,
    reason: String,
) -> (ContractReport, Option<EngineError>) {
    report.reason = reason.clone();

    if is_required(requirement) {
        report.status = "fatal";
        let error = contract_error(requirement, plugins, reason);
        return (report, Some(error));
    }

    report.status = "degraded";
    (report, None)
}

fn emit_contract_line(report: &ContractReport) {
    match report.status {
        "ok" => {
            let parent = newengine_service_api::engine_gateway_parent_id(&report.service_id).unwrap_or_else(|| "<root>".to_owned());
            newengine_ulog_api::ulog::debug!(
                "runtime contract ok: service='{}' parent='{}' provider_service='{}' source='{}' expected='{}'",
                report.service_id,
                parent,
                report.provider_service,
                report.source,
                report.expected,
            );
        }
        "fatal" => {
            newengine_ulog_api::ulog::error!(
                "runtime contract fatal: service='{}' provider='{}' source='{}' expected='{}' reason='{}'",
                report.service_id,
                report.provider,
                report.source,
                report.expected,
                report.reason
            );
        }
        _ => {
            newengine_ulog_api::ulog::warn!(
                "runtime contract degraded: service='{}' provider='{}' source='{}' expected='{}' reason='{}'",
                report.service_id,
                report.provider,
                report.source,
                report.expected,
                report.reason
            );
        }
    }
}

fn emit_runtime_api_table(reports: &[ContractReport]) {
    if newengine_ulog_api::ulog::debug_enabled() {
        let rows = reports
            .iter()
            .map(|report| {
                let root = newengine_service_api::engine_gateway_root_id(&report.service_id).unwrap_or_else(|| report.service_id.clone());
                let parent = newengine_service_api::engine_gateway_parent_id(&report.service_id).unwrap_or_else(|| "<root>".to_owned());
                vec![
                    root,
                    parent,
                    report.service_id.clone(),
                    report.status.to_owned(),
                    report.provider_service.clone(),
                    report.source.clone(),
                    report.capability.clone(),
                    report.required.to_owned(),
                    report.used_by.to_owned(),
                    crate::log_fmt::ellipsize(&report.provider, 72),
                    crate::log_fmt::ellipsize(&report.methods, 64),
                ]
            })
            .collect::<Vec<_>>();

        crate::log_fmt::emit_prefixed_table(
            "runtime api:",
            "Engine API gateway/service contracts",
            &[
                "root",
                "parent",
                "api",
                "status",
                "provider_service",
                "source",
                "capability",
                "strict",
                "used_by",
                "provider",
                "methods",
            ],
            &rows,
        );
        return;
    }

    let ok = reports.iter().filter(|report| report.status == "ok").count();
    let degraded = reports.iter().filter(|report| report.status == "degraded").count();
    let fatal = reports.iter().filter(|report| report.status == "fatal").count();
    newengine_ulog_api::ulog::info!(
        "runtime api: contracts total={} ok={} degraded={} fatal={}",
        reports.len(),
        ok,
        degraded,
        fatal,
    );
}


fn active_route_source(service_id: &str) -> String {
    newengine_plugin_host::active_engine_gateway_route(service_id)
        .map(|route| route.origin)
        .or_else(|| service_origin(service_id))
        .unwrap_or_else(|| {
            if service_id.starts_with("engine.") {
                "missing".to_owned()
            } else {
                "direct".to_owned()
            }
        })
}

fn active_provider_service_id(service_id: &str) -> String {
    newengine_plugin_host::resolve_service_for_engine_gateway(service_id)
        .or_else(|| newengine_plugin_host::has_service(service_id).then(|| service_id.to_owned()))
        .unwrap_or_else(|| "<none>".to_owned())
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
