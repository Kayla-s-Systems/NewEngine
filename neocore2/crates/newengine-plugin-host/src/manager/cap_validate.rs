#![forbid(unsafe_op_in_unsafe_fn)]

use std::sync::atomic::Ordering;

use newengine_plugin_api::{
    capability_has_tag, CapabilityRole, PluginDescriptor, CAPABILITY_TAG_RETIRED,
};

use super::types::{LoadedPlugin, PluginState};
use super::PluginManager;

#[inline]
fn warn_descriptor_retired_capabilities(plugin_id: &str, descriptor: &PluginDescriptor) {
    for capability in descriptor.capabilities.iter() {
        if capability_has_tag(capability, CAPABILITY_TAG_RETIRED) {
            newengine_ulog_api::ulog::warn!(
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
    if crate::host_context::ctx()
        .warned_retired_capabilities
        .swap(true, Ordering::Relaxed)
    {
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
            let candidates = crate::host_context::capability_provider_candidates();

            if newengine_ulog_api::ulog::debug_enabled() {
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

                newengine_ulog_api::ulog::debug!(
                    "plugins: caps validate iter={} providers={} plugins={} disabled={} described={} ",
                    iteration,
                    candidates.len(),
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

                let typed_owned;
                let typed = match p.descriptor_v2.as_ref() {
                    Some(descriptor) => descriptor,
                    None => {
                        typed_owned = newengine_plugin_api::PluginDescriptorV2::from_legacy(d);
                        &typed_owned
                    }
                };
                let missing =
                    crate::host_context::missing_typed_descriptor_requirements(typed, &candidates);
                if !missing.is_empty() {
                    to_disable.push((p.info.id.to_string(), missing));
                } else if newengine_ulog_api::ulog::debug_enabled() {
                    let (prov, req) = count_caps(d);
                    newengine_ulog_api::ulog::debug!(
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
                newengine_ulog_api::ulog::error!(
                    "plugins: disable id='{}' reason='missing required capability(s)' missing=[{}]",
                    id,
                    missing.join(", ")
                );
                let _ = self.disable_by_id(&id, "missing required capability(s)".to_owned());
            }

            // Safety valve: avoid accidental infinite loops if state handling changes.
            if iteration > 32 {
                newengine_ulog_api::ulog::error!(
                    "plugins: capability validation exceeded iteration cap ({}), aborting validation",
                    iteration
                );
                break;
            }
        }
    }
}
