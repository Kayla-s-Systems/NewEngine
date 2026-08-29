use super::*;

pub(crate) fn descriptor_engine_gateways(descriptor: &PluginDescriptor) -> Vec<String> {
    let mut out: Vec<String> = descriptor_gateway_capabilities(descriptor)
        .into_iter()
        .filter_map(|gateway| {
            gateway_provider_service_id(descriptor, &gateway).map(|_| gateway.gateway_id)
        })
        .collect();
    out.sort();
    out.dedup();
    out
}

pub(crate) fn descriptor_max_gateway_priority(descriptor: &PluginDescriptor) -> i32 {
    descriptor_gateway_capabilities(descriptor)
        .into_iter()
        .filter(|gateway| gateway_provider_service_id(descriptor, gateway).is_some())
        .map(|gateway| gateway.backend_priority)
        .max()
        .unwrap_or(0)
}

pub(crate) fn descriptor_engine_gateways_v2(
    descriptor: &newengine_plugin_api::PluginDescriptorV2,
) -> Vec<String> {
    let mut out: Vec<String> =
        crate::service_gateway::metadata::descriptor_gateway_capabilities_v2(descriptor)
            .into_iter()
            .filter_map(|gateway| {
                crate::service_gateway::provider::gateway_provider_service_id_v2(
                    descriptor, &gateway,
                )
                .map(|_| gateway.gateway_id)
            })
            .collect();
    out.sort();
    out.dedup();
    out
}

pub(crate) fn descriptor_max_gateway_priority_v2(
    descriptor: &newengine_plugin_api::PluginDescriptorV2,
) -> i32 {
    crate::service_gateway::metadata::descriptor_gateway_capabilities_v2(descriptor)
        .into_iter()
        .filter(|gateway| {
            crate::service_gateway::provider::gateway_provider_service_id_v2(descriptor, gateway)
                .is_some()
        })
        .map(|gateway| gateway.backend_priority)
        .max()
        .unwrap_or(0)
}
