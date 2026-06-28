use newengine_plugin_api::{CapabilityKind, CapabilityRole, PluginDescriptor};

use super::state::{bump_services_generation, ctx};

#[inline]
pub(crate) fn effective_provider_origin(
    descriptor: &PluginDescriptor,
    default_origin: crate::service_gateway::GatewayProviderOrigin,
) -> crate::service_gateway::GatewayProviderOrigin {
    let id = descriptor.id.as_str().to_ascii_lowercase();
    if id.contains("null") {
        return crate::service_gateway::GatewayProviderOrigin::NullProvider;
    }

    for cap in descriptor.capabilities.iter() {
        if cap.role != CapabilityRole::Provides {
            continue;
        }
        let Ok(value) = serde_json::from_str::<serde_json::Value>(cap.describe_json.as_str())
        else {
            continue;
        };
        let backend_is_null = value
            .get("backend")
            .and_then(|v| v.as_str())
            .map(|v| v.eq_ignore_ascii_case("null"))
            .unwrap_or(false);
        let mode_is_headless = value
            .get("mode")
            .and_then(|v| v.as_str())
            .map(|v| v.eq_ignore_ascii_case("headless"))
            .unwrap_or(false);
        if backend_is_null || mode_is_headless {
            return crate::service_gateway::GatewayProviderOrigin::NullProvider;
        }
    }

    default_origin
}

/// Registers a plugin descriptor (host-owned metadata) for runtime validation.
///
/// Called by the plugin loader *before* `init()` so that service registrations during
/// init can be validated against declared capabilities.
pub(crate) fn register_plugin_descriptor(
    plugin_id: &str,
    d: PluginDescriptor,
    origin: crate::service_gateway::GatewayProviderOrigin,
) -> crate::service_gateway::GatewayProviderOrigin {
    let origin = effective_provider_origin(&d, origin);
    let c = ctx();
    {
        let mut g = match c.plugin_descriptors.lock() {
            Ok(v) => v,
            Err(e) => e.into_inner(),
        };
        g.insert(plugin_id.to_owned(), d);
    }

    {
        let mut g = match c.plugin_origins.lock() {
            Ok(v) => v,
            Err(e) => e.into_inner(),
        };
        g.insert(plugin_id.to_owned(), origin);
    }

    bump_services_generation();
    origin
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub(crate) struct DeclaredCapKey {
    id: String,
    kind: u8,
}

#[inline]
fn declared_cap_key(id: &str, kind: u8) -> DeclaredCapKey {
    DeclaredCapKey {
        id: id.to_owned(),
        kind,
    }
}

pub(crate) fn collect_declared_providers(
    descriptors: impl Iterator<Item = PluginDescriptor>,
) -> newengine_math::collections_prelude::NeHashMap<DeclaredCapKey, u32> {
    let mut out = newengine_math::collections_prelude::NeHashMap::default();
    out.insert(
        declared_cap_key("host.services.v1", CapabilityKind::ServiceV1 as u8),
        1,
    );
    out.insert(
        declared_cap_key("host.events.v1", CapabilityKind::EventsV1 as u8),
        1,
    );

    for d in descriptors {
        for c in d.capabilities.iter() {
            if c.role != CapabilityRole::Provides {
                continue;
            }
            let key = declared_cap_key(c.id.as_str(), c.kind as u8);
            let cur = out.get(&key).copied().unwrap_or(0);
            if c.version > cur {
                out.insert(key, c.version);
            }
        }

        for gateway in crate::service_gateway::descriptor_gateway_capabilities(&d) {
            let _service_kind = gateway.service_kind.as_str();
            if crate::service_gateway::gateway_provider_service_id(&d, &gateway).is_some() {
                let key =
                    declared_cap_key(gateway.gateway_id.as_str(), CapabilityKind::ServiceV1 as u8);
                let cur = out.get(&key).copied().unwrap_or(0);
                if cur < 1 {
                    out.insert(key, 1);
                }
            }
        }
    }

    out
}

pub(crate) fn missing_descriptor_requirements(
    descriptor: &PluginDescriptor,
    providers: &newengine_math::collections_prelude::NeHashMap<DeclaredCapKey, u32>,
) -> Vec<String> {
    let mut out = Vec::new();

    for c in descriptor.capabilities.iter() {
        if c.role != CapabilityRole::Requires {
            continue;
        }

        let key = declared_cap_key(c.id.as_str(), c.kind as u8);
        let pv = providers.get(&key).copied().unwrap_or(0);
        if pv < c.version {
            out.push(format!(
                "{}(kind={} req_v={} avail_v={})",
                c.id, c.kind as u8, c.version, pv
            ));
        }
    }

    out.sort();
    out.dedup();
    out
}

/// Returns:
/// - `Some(true)` if the plugin has a descriptor and declares `Provides(ServiceV1, service_id)`.
/// - `Some(false)` if the plugin has a descriptor but does not declare that capability.
/// - `None` if the plugin has no known descriptor (ABI v1 or loader did not register it).
pub(crate) fn plugin_declares_provided_service(plugin_id: &str, service_id: &str) -> Option<bool> {
    let c = ctx();
    let g = c.plugin_descriptors.lock().ok()?;
    let d = g.get(plugin_id)?;

    for cap in d.capabilities.iter() {
        if cap.role != CapabilityRole::Provides {
            continue;
        }
        if cap.kind != CapabilityKind::ServiceV1 {
            continue;
        }
        if cap.id.as_str() == service_id {
            return Some(true);
        }
    }

    Some(false)
}
