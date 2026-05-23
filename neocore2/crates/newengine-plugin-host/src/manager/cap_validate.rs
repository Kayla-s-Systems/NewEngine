#![forbid(unsafe_op_in_unsafe_fn)]

use std::sync::atomic::{AtomicBool, Ordering};

use newengine_math::collections_prelude::NeHashMap as HashMap;

use newengine_plugin_api::{
    capability_has_tag, CapabilityKind, CapabilityRole, PluginDescriptor, CAPABILITY_TAG_RETIRED,
};

use super::types::{LoadedPlugin, PluginState};
use super::PluginManager;

static WARNED_RETIRED_CAPABILITIES: AtomicBool = AtomicBool::new(false);

#[derive(Clone, PartialEq, Eq, Hash)]
struct CapKey {
    id: String,
    kind: u8,
}

#[inline]
fn cap_key(id: &str, kind: u8) -> CapKey {
    CapKey {
        id: id.to_string(),
        kind,
    }
}

#[inline]
fn collect_providers(loaded: &[LoadedPlugin]) -> HashMap<CapKey, u32> {
    let mut out: HashMap<CapKey, u32> = HashMap::default();

    // Engine/Host baseline capabilities.
    out.insert(cap_key("host.services.v1", CapabilityKind::ServiceV1 as u8), 1);
    out.insert(cap_key("host.events.v1", CapabilityKind::EventsV1 as u8), 1);

    // Registered services and active engine gateways are first-class providers too.
    // This is critical for engine-owned routes such as engine.platform: those routes
    // are not plugin descriptors, but plugin descriptors may legitimately require
    // them before they initialize native resources.
    for service_id in crate::host_context::list_services() {
        let key = cap_key(service_id.as_str(), CapabilityKind::ServiceV1 as u8);
        let cur = out.get(&key).copied().unwrap_or(0);
        if cur < 1 {
            out.insert(key, 1);
        }
    }

    for p in loaded.iter() {
        if p.state == PluginState::Disabled {
            continue;
        }

        let Some(d) = &p.descriptor else {
            continue;
        };

        collect_descriptor_providers(d, &mut out);
    }

    for d in crate::host_context::list_external_runtime_descriptors() {
        collect_descriptor_providers(&d, &mut out);
    }

    out
}

fn collect_descriptor_providers(d: &PluginDescriptor, out: &mut HashMap<CapKey, u32>) {
    for c in d.capabilities.iter() {
        if c.role != CapabilityRole::Provides {
            continue;
        }

        let key = cap_key(c.id.as_str(), c.kind as u8);
        let cur = out.get(&key).copied().unwrap_or(0);
        if c.version > cur {
            out.insert(key, c.version);
        }
    }

    for gateway in crate::service_gateway::descriptor_gateway_capabilities(d) {
        let _service_kind = gateway.service_kind.as_str();
        if crate::service_gateway::gateway_provider_service_id(d, &gateway).is_some() {
            let key = cap_key(gateway.gateway_id.as_str(), CapabilityKind::ServiceV1 as u8);
            let cur = out.get(&key).copied().unwrap_or(0);
            if cur < 1 {
                out.insert(key, 1);
            }
        }
    }
}

#[inline]
fn missing_requirements(d: &PluginDescriptor, providers: &HashMap<CapKey, u32>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();

    for c in d.capabilities.iter() {
        if c.role != CapabilityRole::Requires {
            continue;
        }

        let key = cap_key(c.id.as_str(), c.kind as u8);
        let pv = providers.get(&key).copied().unwrap_or(0);
        if pv < c.version {
            out.push(format!(
                "{}(kind={} req_v={} avail_v={})",
                c.id,
                c.kind as u8,
                c.version,
                pv
            ));
        }
    }

    out.sort();
    out.dedup();
    out
}


#[inline]
fn warn_descriptor_retired_capabilities(plugin_id: &str, descriptor: &PluginDescriptor) {
    for capability in descriptor.capabilities.iter() {
        if capability_has_tag(capability, CAPABILITY_TAG_RETIRED) {
            log::warn!(
                "plugins: capability id='{}' plugin='{}' tag='retired' kind={} role={:?} version={} -- retire or replace this capability declaration",
                capability.id,
                plugin_id,
                capability.kind as u8,
                capability.role,
                capability.version,
            );
        }
    }
}

fn warn_retired_capabilities_once(loaded: &[LoadedPlugin]) {
    if WARNED_RETIRED_CAPABILITIES.swap(true, Ordering::Relaxed) {
        return;
    }

    for plugin in loaded.iter() {
        let Some(descriptor) = &plugin.descriptor else {
            continue;
        };
        warn_descriptor_retired_capabilities(plugin.info.id.as_str(), descriptor);
    }

    for descriptor in crate::host_context::list_external_runtime_descriptors() {
        let plugin_id = descriptor.id.to_string();
        warn_descriptor_retired_capabilities(&plugin_id, &descriptor);
    }
}

#[inline]
fn count_caps(d: &PluginDescriptor) -> (usize, usize) {
    let mut provides = 0usize;
    let mut requires = 0usize;
    for c in d.capabilities.iter() {
        match c.role {
            CapabilityRole::Provides => provides = provides.saturating_add(1),
            CapabilityRole::Requires => requires = requires.saturating_add(1),
        }
    }
    (provides, requires)
}

impl PluginManager {
    /// Validates the loaded plugin set against declared `Requires` capabilities.
    ///
    /// Policy:
    /// - Plugins with unmet requirements are disabled (soft-fail) before `start_all()`.
    /// - Validation is iterated to a fixpoint to handle cascading disables.
    pub(crate) fn validate_required_capabilities(&mut self) {
        warn_retired_capabilities_once(&self.loaded);

        let mut iteration: u32 = 0;

        loop {
            iteration = iteration.saturating_add(1);
            let providers = collect_providers(&self.loaded);

            if log::log_enabled!(log::Level::Debug) {
                let mut checked = 0usize;
                let mut disabled = 0usize;
                let mut with_desc = 0usize;

                for p in self.loaded.iter() {
                    checked = checked.saturating_add(1);
                    if p.state == PluginState::Disabled {
                        disabled = disabled.saturating_add(1);
                    }
                    if p.descriptor.is_some() {
                        with_desc = with_desc.saturating_add(1);
                    }
                }

                log::debug!(
                    "plugins: caps validate iter={} providers={} plugins={} disabled={} described={} ",
                    iteration,
                    providers.len(),
                    checked,
                    disabled,
                    with_desc
                );
            }

            // Collect in deterministic order.
            let mut to_disable: Vec<(String, Vec<String>)> = Vec::new();

            for p in self.loaded.iter() {
                if p.state == PluginState::Disabled {
                    continue;
                }
                let Some(d) = &p.descriptor else {
                    continue;
                };

                let missing = missing_requirements(d, &providers);
                if !missing.is_empty() {
                    to_disable.push((p.info.id.to_string(), missing));
                } else if log::log_enabled!(log::Level::Debug) {
                    let (prov, req) = count_caps(d);
                    log::debug!(
                        "plugins: caps ok id='{}' provides={} requires={} ",
                        p.info.id,
                        prov,
                        req
                    );
                }
            }

            if to_disable.is_empty() {
                break;
            }

            to_disable.sort_by(|a, b| a.0.cmp(&b.0));
            to_disable.dedup_by(|a, b| a.0 == b.0);

            for (id, missing) in to_disable {
                log::error!(
                    "plugins: disable id='{}' reason='missing required capability(s)' missing=[{}]",
                    id,
                    missing.join(", ")
                );
                let _ = self.disable_by_id(&id, "missing required capability(s)".to_owned());
            }

            // Safety valve: avoid accidental infinite loops if state handling changes.
            if iteration > 32 {
                log::error!(
                    "plugins: capability validation exceeded iteration cap ({}), aborting validation",
                    iteration
                );
                break;
            }
        }
    }
}
