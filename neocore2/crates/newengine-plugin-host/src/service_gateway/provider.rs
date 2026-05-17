#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_plugin_api::{CapabilityKind, CapabilityRole, PluginDescriptor};

use super::metadata::EngineGatewayCapability;

pub(crate) fn descriptor_declares_service(
    descriptor: &PluginDescriptor,
    service_id: &str,
) -> bool {
    descriptor.capabilities.iter().any(|cap| {
        cap.role == CapabilityRole::Provides
            && cap.kind == CapabilityKind::ServiceV1
            && cap.id.as_str() == service_id
    })
}

pub(crate) fn descriptor_provided_services(descriptor: &PluginDescriptor) -> Vec<String> {
    let mut out: Vec<String> = descriptor
        .capabilities
        .iter()
        .filter(|cap| cap.role == CapabilityRole::Provides && cap.kind == CapabilityKind::ServiceV1)
        .map(|cap| cap.id.to_string())
        .collect();
    out.sort();
    out.dedup();
    out
}

pub(crate) fn gateway_provider_service_id(
    descriptor: &PluginDescriptor,
    gateway: &EngineGatewayCapability,
) -> Option<String> {
    if let Some(service_id) = gateway.provider_service_id.as_deref() {
        if descriptor_declares_service(descriptor, service_id) {
            return Some(service_id.to_owned());
        }
        log::warn!(
            "plugins: ignoring service gateway because contract service is not declared plugin='{}' engine_gateway='{}' contract='{}' capability='{}'",
            descriptor.id,
            gateway.gateway_id,
            service_id,
            gateway.backend_capability_id
        );
        return None;
    }

    let services = descriptor_provided_services(descriptor);
    match services.as_slice() {
        [single] => Some(single.clone()),
        [] => {
            log::warn!(
                "plugins: ignoring service gateway because provider declares no ServiceV1 plugin='{}' engine_gateway='{}' capability='{}'",
                descriptor.id,
                gateway.gateway_id,
                gateway.backend_capability_id
            );
            None
        }
        _ => {
            log::warn!(
                "plugins: ignoring service gateway because provider declares multiple ServiceV1 entries without contract plugin='{}' engine_gateway='{}' capability='{}' services='{}'",
                descriptor.id,
                gateway.gateway_id,
                gateway.backend_capability_id,
                services.join(",")
            );
            None
        }
    }
}
