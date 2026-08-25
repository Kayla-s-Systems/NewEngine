#![forbid(unsafe_op_in_unsafe_fn)]

use super::provider_route_extends_gateway_parent;
use abi_stable::std_types::ROption;
use newengine_plugin_api::{
    CapabilityDesc, CapabilityDescV2, CapabilityKind, CapabilityRole, PluginDescriptor,
    PluginDescriptorV2,
};
use newengine_service_api::{
    engine_gateway_domain, engine_gateway_matches_service_kind, is_engine_service_gateway_id,
    normalize_service_kind, normalize_system_tag,
};

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
    let typed = capability.to_v2_compat();
    gateway_capability_from_typed(plugin_id, &typed)
}

pub(crate) fn gateway_capability_from_typed(
    plugin_id: &str,
    capability: &CapabilityDescV2,
) -> Option<EngineGatewayCapability> {
    if capability.role != CapabilityRole::Provides || capability.kind == CapabilityKind::ServiceV1 {
        return None;
    }

    let route = match capability.route.clone() {
        ROption::RSome(route) => route,
        ROption::RNone => return None,
    };

    let gateway_id = route.engine_gateway.to_string();
    if !is_engine_service_gateway_id(&gateway_id) {
        newengine_ulog_api::ulog::warn!(
            "plugins: ignoring service gateway with invalid id plugin='{}' capability='{}' engine_gateway='{}'",
            plugin_id,
            capability.id,
            gateway_id
        );
        return None;
    }

    let service_kind_text = route.service_kind.to_string();
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

    let mut system_tags = capability
        .tags
        .iter()
        .filter_map(|tag| normalize_system_tag(tag.as_str()))
        .collect::<Vec<_>>();
    system_tags.sort();
    system_tags.dedup();

    let provider_route_id = match route.provider_route {
        ROption::RSome(value) if !value.trim().is_empty() => Some(value.to_string()),
        _ => None,
    };
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

    let provider_service_id = (!route.provider_service_id.trim().is_empty())
        .then(|| route.provider_service_id.to_string());
    let provider_abi = match route.provider_abi {
        ROption::RSome(value) if !value.trim().is_empty() => Some(value.to_string()),
        _ => None,
    };

    Some(EngineGatewayCapability {
        gateway_id,
        service_kind,
        provider_service_id,
        provider_route_id,
        provider_abi,
        backend_capability_id: capability.id.to_string(),
        backend_priority: route.backend_priority,
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

pub fn descriptor_gateway_capabilities_v2(
    descriptor: &PluginDescriptorV2,
) -> Vec<EngineGatewayCapability> {
    let plugin_id = descriptor.id.to_string();
    descriptor
        .capabilities
        .iter()
        .filter_map(|capability| gateway_capability_from_typed(&plugin_id, capability))
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
    fn direct_v2_gateway_route_uses_typed_fields_without_json() {
        let typed = CapabilityDescV2::new(
            "render.backend.typed",
            CapabilityRole::Provides,
            CapabilityKind::Other,
            2,
        )
        .with_tag("provider.backend")
        .with_route(newengine_plugin_api::BackendRouteDescriptorV2 {
            service_kind: "render".into(),
            engine_gateway: "engine.render".into(),
            provider_service_id: "render.api".into(),
            provider_abi: ROption::RSome("newengine.render-provider/v2".into()),
            provider_route: ROption::RSome("engine.render.typed".into()),
            backend_priority: 99,
            backend: ROption::RSome("typed".into()),
            mode: ROption::RNone,
            features: Vec::new().into(),
        });
        let parsed = gateway_capability_from_typed("test.render", &typed).unwrap();
        assert_eq!(parsed.gateway_id, "engine.render");
        assert_eq!(parsed.backend_priority, 99);
        assert_eq!(parsed.provider_service_id.as_deref(), Some("render.api"));
        assert_eq!(
            parsed.provider_abi.as_deref(),
            Some("newengine.render-provider/v2")
        );
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
