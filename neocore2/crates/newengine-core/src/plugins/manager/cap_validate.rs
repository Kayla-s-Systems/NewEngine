#![forbid(unsafe_op_in_unsafe_fn)]

use std::collections::HashMap;

use newengine_plugin_api::{CapabilityKind, CapabilityRole, PluginDescriptor};

use super::types::{LoadedPlugin, PluginState};
use super::PluginManager;

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
    let mut out: HashMap<CapKey, u32> = HashMap::new();

    // Engine/Host baseline capabilities.
    // These are provided by the host runtime (HostApiV1) regardless of which plugins are present.
    // Without this, any plugin that correctly declares `Requires(host.events.v1)` or
    // `Requires(host.services.v1)` would be disabled during validation.
    out.insert(cap_key("host.services.v1", CapabilityKind::ServiceV1 as u8), 1);
    out.insert(cap_key("host.events.v1", CapabilityKind::EventsV1 as u8), 1);

    for p in loaded.iter() {
        if p.state == PluginState::Disabled {
            continue;
        }

        let Some(d) = &p.descriptor else {
            continue;
        };

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
    }

    out
}

#[inline]
fn first_missing_requirement(d: &PluginDescriptor, providers: &HashMap<CapKey, u32>) -> Option<String> {
    for c in d.capabilities.iter() {
        if c.role != CapabilityRole::Requires {
            continue;
        }

        let key = cap_key(c.id.as_str(), c.kind as u8);
        let pv = providers.get(&key).copied().unwrap_or(0);
        if pv < c.version {
            return Some(format!(
                "missing required capability id='{}' kind={} req_v={} avail_v={}",
                c.id,
                c.kind as u8,
                c.version,
                pv
            ));
        }
    }

    None
}

impl PluginManager {
    /// Validates the loaded plugin set against declared `Requires` capabilities.
    ///
    /// Policy:
    /// - Plugins with unmet requirements are disabled (soft-fail) before `start_all()`.
    /// - Validation is iterated to a fixpoint to handle cascading disables.
    pub(crate) fn validate_required_capabilities(&mut self) {
        let mut iteration: u32 = 0;

        loop {
            iteration = iteration.saturating_add(1);
            let providers = collect_providers(&self.loaded);

            // Collect in deterministic order.
            let mut to_disable: Vec<(String, String)> = Vec::new();

            for p in self.loaded.iter() {
                if p.state == PluginState::Disabled {
                    continue;
                }
                let Some(d) = &p.descriptor else {
                    continue;
                };

                if let Some(reason) = first_missing_requirement(d, &providers) {
                    to_disable.push((p.info.id.to_string(), reason));
                }
            }

            if to_disable.is_empty() {
                break;
            }

            to_disable.sort_by(|a, b| a.0.cmp(&b.0));
            to_disable.dedup_by(|a, b| a.0 == b.0);

            for (id, reason) in to_disable {
                log::error!("plugins: disable id='{}' reason='{}'", id, reason);
                let _ = self.disable_by_id(&id, reason);
            }

            // Safety valve: avoid accidental infinite loops if state handling changes.
            if iteration > 32 {
                log::error!("plugins: capability validation exceeded iteration cap ({}), aborting validation", iteration);
                break;
            }
        }
    }
}
