#![forbid(unsafe_op_in_unsafe_fn)]

//! Declarative plugin-definition helpers.
//!
//! This is the North Star equivalent of the old two-file plugin templates: plugin
//! authors describe identity, services and backend routes in data, while the kit
//! owns descriptor construction and ABI boilerplate.

use crate::plugin_api::{
    BackendRouteDescriptor, BackendServiceSpec, CapabilityDesc, CapabilityKind,
    CapabilityRole, PluginDescriptor, PluginKind,
};
use abi_stable::std_types::RString;
use serde_json::Value;

#[derive(Debug, Clone, Copy)]
pub struct PluginDefinition {
    pub id: &'static str,
    pub name: &'static str,
    pub version: &'static str,
    pub kind: PluginKind,
    pub services: &'static [PluginServiceDefinition],
    pub backend_routes: &'static [PluginBackendRouteDefinition],
    pub capabilities: &'static [PluginCapabilityDefinition],
}

impl PluginDefinition {
    #[inline]
    pub fn descriptor(self) -> PluginDescriptor {
        descriptor_from_definition(self)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct PluginServiceDefinition {
    pub id: &'static str,
    pub version: u32,
    pub describe_json: &'static str,
}

#[derive(Debug, Clone, Copy)]
pub struct PluginBackendRouteDefinition {
    pub capability_id: &'static str,
    pub spec: BackendServiceSpec,
    pub provider_route: Option<&'static str>,
    pub backend: Option<&'static str>,
    pub mode: Option<&'static str>,
    pub priority: i32,
    pub features: &'static [&'static str],
    pub system_tags: &'static [&'static str],
    pub metadata_json: &'static [PluginMetadataJson],
}

#[derive(Debug, Clone, Copy)]
pub struct PluginMetadataJson {
    pub key: &'static str,
    pub value_json: &'static str,
}

#[derive(Debug, Clone, Copy)]
pub struct PluginCapabilityDefinition {
    pub id: &'static str,
    pub role: CapabilityRole,
    pub kind: CapabilityKind,
    pub version: u32,
    pub describe_json: &'static str,
}

#[inline]
pub const fn plugin_service(
    id: &'static str,
    version: u32,
    describe_json: &'static str,
) -> PluginServiceDefinition {
    PluginServiceDefinition { id, version, describe_json }
}

#[inline]
pub const fn metadata_json(key: &'static str, value_json: &'static str) -> PluginMetadataJson {
    PluginMetadataJson { key, value_json }
}

#[inline]
pub const fn provided_capability(
    id: &'static str,
    kind: CapabilityKind,
    version: u32,
    describe_json: &'static str,
) -> PluginCapabilityDefinition {
    PluginCapabilityDefinition {
        id,
        role: CapabilityRole::Provides,
        kind,
        version,
        describe_json,
    }
}

#[inline]
pub const fn optional_backend_route(
    capability_id: &'static str,
    spec: BackendServiceSpec,
    provider_route: Option<&'static str>,
    backend: Option<&'static str>,
    mode: Option<&'static str>,
    priority: i32,
    features: &'static [&'static str],
    system_tags: &'static [&'static str],
    metadata_json: &'static [PluginMetadataJson],
) -> PluginBackendRouteDefinition {
    PluginBackendRouteDefinition {
        capability_id,
        spec,
        provider_route,
        backend,
        mode,
        priority,
        features,
        system_tags,
        metadata_json,
    }
}

#[inline]
pub fn descriptor_from_definition(def: PluginDefinition) -> PluginDescriptor {
    let mut builder = PluginDescriptor::builder(def.id, def.name, def.version, def.kind);

    for service in def.services {
        builder = builder.provides_service(
            service.id,
            service.version,
            RString::from(service.describe_json),
        );
    }

    for route in def.backend_routes {
        let mut desc = BackendRouteDescriptor::new(route.spec).priority(route.priority);
        if let Some(provider_route) = route.provider_route {
            desc = desc.provider_route(provider_route);
        }
        if let Some(backend) = route.backend {
            desc = desc.backend(backend);
        }
        if let Some(mode) = route.mode {
            desc = desc.mode(mode);
        }
        if !route.features.is_empty() {
            desc = desc.features(route.features.iter().copied());
        }
        if !route.system_tags.is_empty() {
            desc = desc.system_tags(route.system_tags.iter().copied());
        }
        for item in route.metadata_json {
            let value = parse_metadata_value(item.key, item.value_json);
            desc = desc.metadata_json(item.key, value);
        }
        builder = builder.push(CapabilityDesc::backend_route(route.capability_id, desc));
    }

    for cap in def.capabilities {
        builder = builder.push(
            CapabilityDesc::new(cap.id, cap.role, cap.kind, cap.version)
                .with_json(RString::from(cap.describe_json)),
        );
    }

    builder.build()
}

fn parse_metadata_value(key: &str, raw: &str) -> Value {
    serde_json::from_str(raw).unwrap_or_else(|e| {
        panic!("invalid plugin metadata JSON for key '{key}': {e}: {raw}")
    })
}
