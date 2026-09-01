#![forbid(unsafe_op_in_unsafe_fn)]

mod control;
mod diagnostics;
mod load;

use super::Engine;

use crate::error::{EngineError, EngineResult};
use newengine_plugin_host::PluginsSnapshot;
use std::sync::atomic::{AtomicU64, Ordering};

static PLUGIN_SNAPSHOT_REVISION: AtomicU64 = AtomicU64::new(0);

#[inline]
fn bootstrap_preload_deferred() -> bool {
    newengine_plugin_host::current_host_context()
        .environment_var("NEWENGINE_BOOTSTRAP_PLUGIN_PRELOAD")
        .map(|v| {
            matches!(
                v.trim().to_ascii_lowercase().as_str(),
                "0" | "false" | "off" | "defer" | "deferred" | "safe"
            )
        })
        .unwrap_or(false)
}

impl<E: Send + 'static> Engine<E> {
    #[inline]
    pub(crate) fn expose_plugins_snapshot(&mut self) {
        let revision = PLUGIN_SNAPSHOT_REVISION
            .fetch_add(1, Ordering::AcqRel)
            .saturating_add(1);
        self.resources.insert(PluginsSnapshot {
            revision,
            plugins: self.plugins.snapshot().into(),
        });
    }

    #[inline]
    pub(crate) fn plugins_start_all(&mut self) -> EngineResult<()> {
        self.plugins
            .start_all()
            .map_err(|e| EngineError::Other(format!("plugins: start failed: {e}")))?;

        // Plugin lifecycle is retained runtime state. Loading publishes a snapshot while
        // providers are still Registered; start_all() then transitions them to Running.
        // Republish immediately so capability-driven consumers (editor UI, previews, etc.)
        // never observe the stale pre-start snapshot for the lifetime of the process.
        self.expose_plugins_snapshot();
        Ok(())
    }

    #[inline]
    pub(crate) fn plugins_shutdown(&mut self) {
        self.plugins.shutdown();
    }
}

impl<E: Send + 'static> Engine<E> {
    pub(super) fn resolved_plugin_discovery_roots(
        &self,
    ) -> EngineResult<Vec<super::PluginDiscoveryRoot>> {
        let mut out = Vec::new();
        if self.plugins_dir.is_some() || self.implicit_plugin_discovery {
            let primary_dir =
                newengine_plugin_host::resolve_plugin_discovery_dir(self.plugins_dir.as_deref())
                    .map_err(|error| {
                        EngineError::Other(format!(
                            "plugins: primary discovery root resolve failed: {error}"
                        ))
                    })?;
            out.push(
                super::PluginDiscoveryRoot::new(
                    primary_dir,
                    newengine_plugin_host::PluginLoadOrigin::FirstPartyPlugin,
                )
                .required(true)
                .with_owner("engine.startup"),
            );
        }

        for root in &self.plugin_roots {
            if !root.dir.is_dir() {
                if root.required {
                    return Err(EngineError::Other(format!(
                        "plugins: required discovery root missing owner='{}' origin='{}' dir='{}'",
                        root.owner,
                        root.origin.as_str(),
                        root.dir.display(),
                    )));
                }
                newengine_ulog_api::ulog::debug!(
                    "plugins: optional discovery root skipped owner='{}' origin='{}' dir='{}'",
                    root.owner,
                    root.origin.as_str(),
                    root.dir.display(),
                );
                continue;
            }
            let dir = newengine_plugin_host::resolve_plugin_discovery_dir(Some(&root.dir))
                .map_err(|error| {
                    EngineError::Other(format!(
                        "plugins: discovery root resolve failed owner='{}' dir='{}': {error}",
                        root.owner,
                        root.dir.display(),
                    ))
                })?;
            if out
                .iter()
                .any(|existing: &super::PluginDiscoveryRoot| existing.dir == dir)
            {
                newengine_ulog_api::ulog::warn!(
                    "plugins: duplicate discovery root ignored owner='{}' origin='{}' dir='{}'",
                    root.owner,
                    root.origin.as_str(),
                    dir.display(),
                );
                continue;
            }
            let mut resolved = root.clone();
            resolved.dir = dir;
            out.push(resolved);
        }
        Ok(out)
    }
}

impl<E: Send + 'static> Engine<E> {
    /// Descriptor-only plugin runtime-unit inventory scan. No plugin is initialized here.
    /// This is consumed before runtime-unit solving so plugin units participate in the same
    /// catalog as distribution/profile/game units.
    pub fn scan_plugin_runtime_unit_inventory(
        &self,
    ) -> EngineResult<Vec<newengine_plugin_host::PluginRuntimeUnitInventoryEntry>> {
        let roots = self.resolved_plugin_discovery_roots()?;
        let mut out = Vec::new();
        for root in roots {
            match newengine_plugin_host::scan_plugin_discovery_graph(&root.dir) {
                Ok(graph) => {
                    let mut units = graph.runtime_unit_inventory().map_err(|error| {
                        EngineError::Other(format!(
                            "plugins: runtime-unit inventory parse failed owner='{}' root='{}': {}",
                            root.owner,
                            root.dir.display(),
                            error
                        ))
                    })?;
                    out.append(&mut units);
                }
                Err(error) if root.required => {
                    return Err(EngineError::Other(format!(
                        "plugins: required runtime-unit inventory root scan failed owner='{}' root='{}': {}",
                        root.owner,
                        root.dir.display(),
                        error
                    )));
                }
                Err(error) => {
                    newengine_ulog_api::ulog::warn!(
                        "plugins: optional runtime-unit inventory root skipped owner='{}' root='{}' err={}",
                        root.owner,
                        root.dir.display(),
                        error
                    );
                }
            }
        }
        Ok(out)
    }
}

impl<E: Send + 'static> Engine<E> {
    pub(super) fn validate_required_plugins_loaded(&self) -> EngineResult<()> {
        if self.required_plugin_ids.is_empty() {
            return Ok(());
        }
        let missing = self
            .required_plugin_ids
            .iter()
            .filter(|id| !self.plugins.has_plugin(id))
            .cloned()
            .collect::<Vec<_>>();
        if missing.is_empty() {
            return Ok(());
        }
        Err(EngineError::Other(format!(
            "plugins: required plugin id(s) missing after all discovery roots: [{}]",
            missing.join(", ")
        )))
    }
}
