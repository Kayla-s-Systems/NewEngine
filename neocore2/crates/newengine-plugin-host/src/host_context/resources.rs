use newengine_plugin_api::{PluginDescriptor, PluginInfo};
use newengine_ulog_api::path_format::canonicalize_if_exists;
use std::path::PathBuf;

use super::state::{ctx, ExternalRuntimePluginEntry, ExternalRuntimePluginSnapshot};
use super::validation::{
    capability_provider_candidates_with_descriptor, effective_provider_origin,
    missing_typed_descriptor_requirements,
};

pub fn register_external_runtime_plugin(
    path: PathBuf,
    info: PluginInfo,
    descriptor: PluginDescriptor,
    state: impl Into<String>,
) -> Result<(), String> {
    crate::host_context::reject_topology_mutation_from_host_callback(
        "register_external_runtime_plugin",
    )?;
    let plugin_id = info.id.to_string();
    if plugin_id.trim().is_empty() {
        return Err("external runtime plugin id is empty".to_owned());
    }
    if descriptor.id.as_str() != plugin_id {
        return Err(format!(
            "external runtime plugin descriptor id mismatch info='{}' descriptor='{}'",
            plugin_id, descriptor.id
        ));
    }

    let c = ctx();
    let descriptor_v2 = newengine_plugin_api::PluginDescriptorV2::from_legacy(&descriptor);
    let candidates = capability_provider_candidates_with_descriptor(&descriptor_v2);
    let missing = missing_typed_descriptor_requirements(&descriptor_v2, &candidates);
    if !missing.is_empty() {
        return Err(format!(
            "missing required capability(s) for external runtime plugin id='{}': [{}]",
            plugin_id,
            missing.join(", ")
        ));
    }

    let contract_count =
        newengine_runtime_contract_catalog::contracts_from_plugin_descriptor(&descriptor)?.len();
    let normalized_path = canonicalize_if_exists(&path);
    let origin = effective_provider_origin(
        &descriptor,
        Some(&descriptor_v2),
        crate::service_gateway::GatewayProviderOrigin::from_plugin_path(&normalized_path),
    );
    let runtime_state = state.into();

    super::lifecycle::begin_provider_transaction(&plugin_id)?;
    let staged = super::lifecycle::stage_plugin_descriptor_registration(
        &plugin_id,
        descriptor.clone(),
        None,
        origin,
    );
    if let Err(error) = staged {
        super::lifecycle::rollback_provider_transaction(&plugin_id);
        return Err(error);
    }
    if let Err(error) = super::lifecycle::validate_provider_transaction(&plugin_id) {
        super::lifecycle::rollback_provider_transaction(&plugin_id);
        return Err(error);
    }
    super::lifecycle::commit_provider_transaction(&plugin_id)?;

    {
        let mut runtimes = match c.external_runtime_plugins.lock() {
            Ok(v) => v,
            Err(e) => e.into_inner(),
        };
        runtimes.insert(
            plugin_id.clone(),
            ExternalRuntimePluginEntry {
                path: normalized_path.clone(),
                info: info.clone(),
                descriptor: descriptor.clone(),
                state: runtime_state,
            },
        );
    }

    newengine_ulog_api::ulog::info!(
        "plugins: external runtime registered id='{}' ver='{}' kind={:?} origin='{}' contracts={} transaction='stage-validate-commit' path='{}'",
        plugin_id,
        info.version,
        descriptor.kind,
        origin.as_str(),
        contract_count,
        newengine_ulog_api::path_format::display_clean(&normalized_path)
    );

    Ok(())
}

pub fn list_external_runtime_plugins() -> Vec<ExternalRuntimePluginSnapshot> {
    let c = ctx();
    let g = match c.external_runtime_plugins.lock() {
        Ok(v) => v,
        Err(e) => e.into_inner(),
    };

    let mut out: Vec<ExternalRuntimePluginSnapshot> = g
        .values()
        .map(|entry| ExternalRuntimePluginSnapshot {
            path: entry.path.clone(),
            id: entry.info.id.to_string(),
            name: entry.info.name.to_string(),
            version: entry.info.version.to_string(),
            kind: Some(entry.descriptor.kind),
            capabilities: entry.descriptor.capabilities.iter().cloned().collect(),
            state: entry.state.clone(),
            disabled_reason: None,
        })
        .collect();
    out.sort_by(|a, b| a.id.cmp(&b.id));
    out
}

pub fn list_external_runtime_descriptors() -> Vec<PluginDescriptor> {
    let c = ctx();
    let g = match c.external_runtime_plugins.lock() {
        Ok(v) => v,
        Err(e) => e.into_inner(),
    };
    let mut out: Vec<PluginDescriptor> = g.values().map(|entry| entry.descriptor.clone()).collect();
    out.sort_by(|a, b| a.id.cmp(&b.id));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn external_runtime_contracts_use_provider_transaction() {
        let context = crate::host_context::create_host_context();
        crate::host_context::activate_host_context(&context);
        let plugin_id = "test.external.runtime";
        let descriptor = newengine_plugin_api::PluginDescriptor::builder(
            plugin_id,
            "External Runtime",
            "1.0.0",
            newengine_plugin_api::PluginKind::Runtime,
        )
        .push(
            newengine_plugin_api::RuntimeContractDeclaration::new(
                "test.external.protocol",
                newengine_plugin_api::ContractKind::Protocol,
                newengine_plugin_api::ContractVersion::major(1),
                newengine_plugin_api::ContractCompatibility::SameMajor,
            )
            .advertised_id("test.external/v1")
            .into_capability(),
        )
        .build();
        let info = newengine_plugin_api::PluginInfo {
            id: plugin_id.into(),
            name: "External Runtime".into(),
            version: "1.0.0".into(),
        };

        register_external_runtime_plugin(
            std::path::PathBuf::from("test-external-runtime.dll"),
            info,
            descriptor,
            "running",
        )
        .unwrap();

        let contract = crate::host_context::runtime_contract("test.external.protocol").unwrap();
        assert_eq!(contract.spec.owner, plugin_id);
        assert!(
            crate::host_context::runtime_contract_by_advertised_id("test.external/v1").is_some()
        );

        crate::host_context::unregister_by_owner(plugin_id);
        assert!(crate::host_context::runtime_contract("test.external.protocol").is_none());
    }
}
