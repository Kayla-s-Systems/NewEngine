#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_plugin_api::{CapabilityDesc, CapabilityKind, CapabilityRole, PluginDescriptor};
use newengine_service_api::{
    engine_gateway_domain, is_engine_service_gateway_id, normalize_service_kind, normalize_system_tag,
};

pub(crate) const ENGINE_GATEWAY_FIELD: &str = "engine_gateway";
pub(crate) const SERVICE_KIND_FIELD: &str = "service_kind";
pub(crate) const CONTRACT_FIELD: &str = "contract";
pub(crate) const BACKEND_PRIORITY_FIELD: &str = "backend_priority";
pub(crate) const SYSTEM_TAGS_FIELD: &str = "system_tags";
pub(crate) const TAGS_FIELD: &str = "tags";

#[derive(Debug, Clone)]
pub(crate) struct EngineGatewayCapability {
    pub(crate) gateway_id: String,
    pub(crate) service_kind: String,
    pub(crate) provider_service_id: Option<String>,
    pub(crate) backend_capability_id: String,
    pub(crate) backend_priority: i32,
    pub(crate) system_tags: Vec<String>,
}

#[inline]
fn json_field_string(value: &serde_json::Value, field: &str) -> Option<String> {
    value
        .get(field)
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(str::to_owned)
}

#[inline]
fn json_field_i32(value: &serde_json::Value, field: &str) -> Option<i32> {
    value
        .get(field)
        .and_then(|v| v.as_i64())
        .map(|v| v.clamp(i32::MIN as i64, i32::MAX as i64) as i32)
}

fn json_string_list(value: &serde_json::Value, field: &str) -> Vec<String> {
    match value.get(field) {
        Some(serde_json::Value::Array(values)) => values
            .iter()
            .filter_map(|v| v.as_str())
            .filter_map(normalize_system_tag)
            .collect(),
        Some(serde_json::Value::String(value)) => normalize_system_tag(value).into_iter().collect(),
        _ => Vec::new(),
    }
}

fn json_system_tags(value: &serde_json::Value) -> Vec<String> {
    let mut out = json_string_list(value, SYSTEM_TAGS_FIELD);
    out.extend(json_string_list(value, TAGS_FIELD));
    out.sort();
    out.dedup();
    out
}

#[inline]
fn service_kind_matches_gateway(service_kind: &str, gateway_id: &str) -> bool {
    engine_gateway_domain(gateway_id).as_deref() == Some(service_kind)
}

pub(crate) fn gateway_capability_from_capability(
    plugin_id: &str,
    capability: &CapabilityDesc,
) -> Option<EngineGatewayCapability> {
    if capability.role != CapabilityRole::Provides || capability.kind == CapabilityKind::ServiceV1 {
        return None;
    }

    let raw = capability.describe_json.as_str().trim();
    if raw.is_empty() {
        return None;
    }

    let value = match serde_json::from_str::<serde_json::Value>(raw) {
        Ok(value) => value,
        Err(err) => {
            log::warn!(
                "plugins: ignoring malformed service capability metadata plugin='{}' capability='{}' error='{}'",
                plugin_id,
                capability.id,
                err
            );
            return None;
        }
    };

    let Some(gateway_id) = json_field_string(&value, ENGINE_GATEWAY_FIELD) else {
        return None;
    };

    if !is_engine_service_gateway_id(&gateway_id) {
        log::warn!(
            "plugins: ignoring service gateway with invalid id plugin='{}' capability='{}' engine_gateway='{}'",
            plugin_id,
            capability.id,
            gateway_id
        );
        return None;
    }

    let Some(service_kind_text) = json_field_string(&value, SERVICE_KIND_FIELD) else {
        log::warn!(
            "plugins: ignoring service gateway without service_kind plugin='{}' capability='{}' engine_gateway='{}'",
            plugin_id,
            capability.id,
            gateway_id
        );
        return None;
    };

    let Some(service_kind) = normalize_service_kind(&service_kind_text) else {
        log::warn!(
            "plugins: ignoring service gateway with invalid service_kind plugin='{}' capability='{}' service_kind='{}' engine_gateway='{}'",
            plugin_id,
            capability.id,
            service_kind_text,
            gateway_id
        );
        return None;
    };

    if !service_kind_matches_gateway(&service_kind, &gateway_id) {
        log::warn!(
            "plugins: ignoring service gateway with mixed domain levels plugin='{}' capability='{}' service_kind='{}' engine_gateway='{}' gateway_domain='{}'",
            plugin_id,
            capability.id,
            service_kind,
            gateway_id,
            engine_gateway_domain(&gateway_id).unwrap_or_else(|| "<invalid>".to_owned())
        );
        return None;
    }

    Some(EngineGatewayCapability {
        gateway_id,
        service_kind,
        provider_service_id: json_field_string(&value, CONTRACT_FIELD),
        backend_capability_id: capability.id.to_string(),
        backend_priority: json_field_i32(&value, BACKEND_PRIORITY_FIELD).unwrap_or(0),
        system_tags: json_system_tags(&value),
    })
}

pub(crate) fn descriptor_gateway_capabilities(
    descriptor: &PluginDescriptor,
) -> Vec<EngineGatewayCapability> {
    let plugin_id = descriptor.id.to_string();
    descriptor
        .capabilities
        .iter()
        .filter_map(|capability| gateway_capability_from_capability(&plugin_id, capability))
        .collect()
}
