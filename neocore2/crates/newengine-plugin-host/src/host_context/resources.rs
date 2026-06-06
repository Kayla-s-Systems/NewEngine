use crate::path_fmt::canonicalize_if_exists;
use newengine_plugin_api::{PluginDescriptor, PluginInfo};
use std::path::PathBuf;

use super::state::{bump_services_generation, ctx, ExternalRuntimePluginEntry, ExternalRuntimePluginSnapshot};
use super::validation::{collect_declared_providers, effective_provider_origin, missing_descriptor_requirements};

pub fn register_external_runtime_plugin(
    path: PathBuf,
    info: PluginInfo,
    descriptor: PluginDescriptor,
    state: impl Into<String>,
) -> Result<(), String> {
    let plugin_id = info.id.to_string();
    if plugin_id.trim().is_empty() {
        return Err("external runtime plugin id is empty".to_owned());
    }

    let c = ctx();

    let providers = {
        let g = match c.plugin_descriptors.lock() {
            Ok(v) => v,
            Err(e) => e.into_inner(),
        };
        let mut descriptors: Vec<PluginDescriptor> = g.values().cloned().collect();
        descriptors.push(descriptor.clone());
        collect_declared_providers(descriptors.into_iter())
    };

    let missing = missing_descriptor_requirements(&descriptor, &providers);
    if !missing.is_empty() {
        return Err(format!(
            "missing required capability(s) for external runtime plugin id='{}': [{}]",
            plugin_id,
            missing.join(", ")
        ));
    }

    let normalized_path = canonicalize_if_exists(&path);
    let origin = effective_provider_origin(
        &descriptor,
        crate::service_gateway::GatewayProviderOrigin::from_plugin_path(&normalized_path),
    );

    {
        let mut descriptors = match c.plugin_descriptors.lock() {
            Ok(v) => v,
            Err(e) => e.into_inner(),
        };
        descriptors.insert(plugin_id.clone(), descriptor.clone());
    }

    {
        let mut origins = match c.plugin_origins.lock() {
            Ok(v) => v,
            Err(e) => e.into_inner(),
        };
        origins.insert(plugin_id.clone(), origin);
    }

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
                state: state.into(),
            },
        );
    }

    bump_services_generation();

    newengine_ulog_api::ulog::info!(
        "plugins: external runtime registered id='{}' ver='{}' kind={:?} origin='{}' path='{}'",
        plugin_id,
        info.version,
        descriptor.kind,
        origin.as_str(),
        crate::path_fmt::display_clean(&normalized_path)
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
