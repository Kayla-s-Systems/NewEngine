#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_plugin_api::{CapabilityKind, CapabilityRole, PluginDescriptor, PluginDescriptorV2};

use super::metadata::EngineGatewayCapability;

fn warn_once(key: String, message: impl FnOnce()) {
    let context = crate::host_context::ctx();
    let should_log = match context.invalid_gateway_route_warnings.lock() {
        Ok(mut set) => set.insert(key),
        Err(poisoned) => poisoned.into_inner().insert(key),
    };
    if should_log {
        message();
    }
}

pub(crate) fn descriptor_declares_service(descriptor: &PluginDescriptor, service_id: &str) -> bool {
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

pub(crate) fn descriptor_v2_declares_service(
    descriptor: &PluginDescriptorV2,
    service_id: &str,
) -> bool {
    descriptor.capabilities.iter().any(|cap| {
        cap.role == CapabilityRole::Provides
            && cap.kind == CapabilityKind::ServiceV1
            && cap.id.as_str() == service_id
    })
}

pub(crate) fn descriptor_v2_provided_services(descriptor: &PluginDescriptorV2) -> Vec<String> {
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
        warn_once(
            format!(
                "contract-not-declared:{}:{}:{}:{}",
                descriptor.id, gateway.gateway_id, service_id, gateway.backend_capability_id
            ),
            || {
                newengine_ulog_api::ulog::warn!(
                    "plugins: ignoring service gateway because contract service is not declared plugin='{}' engine_gateway='{}' contract='{}' capability='{}'",
                    descriptor.id,
                    gateway.gateway_id,
                    service_id,
                    gateway.backend_capability_id
                );
            },
        );
        return None;
    }

    let services = descriptor_provided_services(descriptor);
    match services.as_slice() {
        [single] => Some(single.clone()),
        [] => {
            warn_once(
                format!(
                    "no-service:{}:{}:{}",
                    descriptor.id, gateway.gateway_id, gateway.backend_capability_id
                ),
                || {
                    newengine_ulog_api::ulog::warn!(
                        "plugins: ignoring service gateway because provider declares no ServiceV1 plugin='{}' engine_gateway='{}' capability='{}'",
                        descriptor.id,
                        gateway.gateway_id,
                        gateway.backend_capability_id
                    );
                },
            );
            None
        }
        _ => {
            let services_joined = services.join(",");
            warn_once(
                format!(
                    "multi-service:{}:{}:{}:{}",
                    descriptor.id,
                    gateway.gateway_id,
                    gateway.backend_capability_id,
                    services_joined
                ),
                || {
                    newengine_ulog_api::ulog::warn!(
                        "plugins: ignoring service gateway because provider declares multiple ServiceV1 entries without contract plugin='{}' engine_gateway='{}' capability='{}' services='{}'",
                        descriptor.id,
                        gateway.gateway_id,
                        gateway.backend_capability_id,
                        services_joined
                    );
                },
            );
            None
        }
    }
}

pub(crate) fn gateway_provider_service_id_v2(
    descriptor: &PluginDescriptorV2,
    gateway: &EngineGatewayCapability,
) -> Option<String> {
    if let Some(service_id) = gateway.provider_service_id.as_deref() {
        if descriptor_v2_declares_service(descriptor, service_id) {
            return Some(service_id.to_owned());
        }
        return None;
    }

    let services = descriptor_v2_provided_services(descriptor);
    match services.as_slice() {
        [single] => Some(single.clone()),
        _ => None,
    }
}
