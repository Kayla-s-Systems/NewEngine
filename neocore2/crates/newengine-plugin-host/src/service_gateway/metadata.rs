#![forbid(unsafe_op_in_unsafe_fn)]

use super::provider_route_extends_gateway_parent;
use newengine_plugin_api::{CapabilityDesc, CapabilityKind, CapabilityRole, PluginDescriptor};
use newengine_service_api::{
    engine_gateway_domain, engine_gateway_matches_service_kind, is_engine_service_gateway_id,
    normalize_service_kind, normalize_system_tag,
};

pub(crate) const ENGINE_GATEWAY_FIELD: &str = "engine_gateway";
pub(crate) const SERVICE_KIND_FIELD: &str = "service_kind";
pub(crate) const CONTRACT_FIELD: &str = "contract";
pub(crate) const PROVIDER_ROUTE_FIELD: &str = "provider_route";
pub(crate) const PROVIDER_ABI_FIELD: &str = "provider_abi";
pub(crate) const BACKEND_PRIORITY_FIELD: &str = "backend_priority";
pub(crate) const BACKEND_FIELD: &str = "backend";
pub(crate) const FEATURES_FIELD: &str = "features";
pub(crate) const SYSTEM_TAGS_FIELD: &str = "system_tags";
pub(crate) const TAGS_FIELD: &str = "tags";

#[derive(Debug, Clone)]
pub struct EngineGatewayCapability {
    pub gateway_id: String,
    pub service_kind: String,
    pub provider_service_id: Option<String>,
    pub provider_route_id: Option<String>,
    pub provider_abi: Option<String>,
    pub backend_capability_id: String,
    pub backend_priority: i32,
    pub system_tags: Vec<String>,
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
    if let Some(backend) = json_field_string(value, BACKEND_FIELD) {
        if let Some(tag) = normalize_system_tag(&format!("backend.{}", metadata_tag_slug(&backend))) {
            out.push(tag);
        }
    }
    match value.get(FEATURES_FIELD) {
        Some(serde_json::Value::Array(features)) => {
            for feature in features.iter().filter_map(|value| value.as_str()) {
                if let Some(tag) = normalize_system_tag(&format!("feature.{}", metadata_tag_slug(feature))) {
                    out.push(tag);
                }
            }
        }
        Some(serde_json::Value::String(feature)) => {
            if let Some(tag) = normalize_system_tag(&format!("feature.{}", metadata_tag_slug(feature))) {
                out.push(tag);
            }
        }
        _ => {}
    }
    out.sort();
    out.dedup();
    out
}

fn metadata_tag_slug(value: &str) -> String {
    value
        .trim()
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch.to_ascii_lowercase() } else { '.' })
        .collect::<String>()
        .split('.')
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>()
        .join(".")
}

#[inline]
fn service_kind_matches_gateway(
    service_kind: &str,
    gateway_id: &str,
    _system_tags: &[String],
) -> bool {
    // `engine_gateway` is the actual backend API route. Provider personal
    // identities such as `engine.ui.<provider>` belong in descriptor metadata
    // (`provider_route`), not in the gateway tree.
    engine_gateway_matches_service_kind(gateway_id, service_kind)
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
            newengine_ulog_api::ulog::warn!(
                "plugins: ignoring malformed service capability metadata plugin='{}' capability='{}' error='{}'",
                plugin_id,
                capability.id,
                err
            );
            return None;
        }
    };

    let gateway_id = json_field_string(&value, ENGINE_GATEWAY_FIELD)?;

    if !is_engine_service_gateway_id(&gateway_id) {
        newengine_ulog_api::ulog::warn!(
            "plugins: ignoring service gateway with invalid id plugin='{}' capability='{}' engine_gateway='{}'",
            plugin_id,
            capability.id,
            gateway_id
        );
        return None;
    }

    let Some(service_kind_text) = json_field_string(&value, SERVICE_KIND_FIELD) else {
        newengine_ulog_api::ulog::warn!(
            "plugins: ignoring service gateway without service_kind plugin='{}' capability='{}' engine_gateway='{}'",
            plugin_id,
            capability.id,
            gateway_id
        );
        return None;
    };

    let Some(service_kind) = normalize_service_kind(&service_kind_text) else {
        newengine_ulog_api::ulog::warn!(
            "plugins: ignoring service gateway with invalid service_kind plugin='{}' capability='{}' service_kind='{}' engine_gateway='{}'",
            plugin_id,
            capability.id,
            service_kind_text,
            gateway_id
        );
        return None;
    };

    let system_tags = json_system_tags(&value);
    let provider_route_id = json_field_string(&value, PROVIDER_ROUTE_FIELD);
    if let Some(provider_route_id) = provider_route_id.as_deref() {
        if !is_engine_service_gateway_id(provider_route_id)
            || !provider_route_extends_gateway_parent(&gateway_id, provider_route_id)
        {
            newengine_ulog_api::ulog::warn!(
                "plugins: ignoring service gateway with invalid provider_route plugin='{}' capability='{}' engine_gateway='{}' provider_route='{}'",
                plugin_id,
                capability.id,
                gateway_id,
                provider_route_id
            );
            return None;
        }
    }

    if !service_kind_matches_gateway(&service_kind, &gateway_id, &system_tags) {
        newengine_ulog_api::ulog::warn!(
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
        provider_route_id,
        provider_abi: json_field_string(&value, PROVIDER_ABI_FIELD),
        backend_capability_id: capability.id.to_string(),
        backend_priority: json_field_i32(&value, BACKEND_PRIORITY_FIELD).unwrap_or(0),
        system_tags,
    })
}

pub fn descriptor_gateway_capabilities(
    descriptor: &PluginDescriptor,
) -> Vec<EngineGatewayCapability> {
    let plugin_id = descriptor.id.to_string();
    descriptor
        .capabilities
        .iter()
        .filter_map(|capability| gateway_capability_from_capability(&plugin_id, capability))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn capability(json: &str) -> CapabilityDesc {
        CapabilityDesc::new(
            "render.backend",
            CapabilityRole::Provides,
            CapabilityKind::Other,
            1,
        )
        .with_json(json)
    }

    #[test]
    fn provider_abi_is_optional_for_legacy_descriptor_metadata() {
        let cap = capability(
            r#"{"service_kind":"render","engine_gateway":"engine.render","provider_route":"engine.render.test","contract":"render.api","backend_priority":1}"#,
        );
        let parsed = gateway_capability_from_capability("test.render", &cap).unwrap();
        assert_eq!(parsed.provider_abi, None);
    }

    #[test]
    fn provider_abi_is_preserved_when_advertised() {
        let cap = capability(
            r#"{"service_kind":"render","engine_gateway":"engine.render","provider_route":"engine.render.test","provider_abi":"newengine.render-provider/v1","contract":"render.api","backend_priority":1}"#,
        );
        let parsed = gateway_capability_from_capability("test.render", &cap).unwrap();
        assert_eq!(
            parsed.provider_abi.as_deref(),
            Some("newengine.render-provider/v1")
        );
    }
}
