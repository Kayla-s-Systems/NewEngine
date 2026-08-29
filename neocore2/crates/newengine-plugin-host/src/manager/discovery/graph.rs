#![forbid(unsafe_op_in_unsafe_fn)]

use std::path::PathBuf;

use newengine_plugin_api::{PluginBootstrapPhase, PluginKind};

#[derive(Debug, Clone)]
pub(super) enum ScannedDynlibKind {
    PlatformRuntime {
        id: String,
        version: String,
        system_tags: Vec<String>,
        // Descriptor diagnostic/ranking metadata; windowed runtime performs final runtime ranking.
        #[allow(dead_code)]
        backend_priority: i32,
    },
    Plugin {
        id: String,
        version: String,
        phase: PluginBootstrapPhase,
        descriptor_kind: Option<PluginKind>,
        declared_capabilities: Option<usize>,
        descriptor: Option<newengine_plugin_api::PluginDescriptor>,
        descriptor_v2: Option<newengine_plugin_api::PluginDescriptorV2>,
        service_gateways: Vec<String>,
        // Descriptor diagnostic metadata only; never an authority before frozen composition.
        #[allow(dead_code)]
        backend_priority: i32,
    },
    Unknown,
}

#[derive(Debug, Clone)]
pub(super) struct ScannedDynlib {
    pub(super) path: PathBuf,
    pub(super) file_name: String,
    /// Verified sidecar snapshot captured during discovery. It is immutable input
    /// to the authoritative composition/load plan and must not be re-read later.
    pub(super) discovery_manifest: Option<super::sidecar::VerifiedPluginDiscoveryManifest>,
    pub(super) kind: ScannedDynlibKind,
}

#[derive(Debug, Clone)]
pub struct DiscoveryGraph {
    pub(super) dir: PathBuf,
    pub(super) entries_total: usize,
    pub(super) skipped_non_dynlib: usize,
    pub(super) items: Vec<ScannedDynlib>,
    pub(super) scan_errors: Vec<String>,
    pub(super) platform_runtime_count: usize,
    pub(super) bootstrap_total: usize,
    pub(super) engine_total: usize,
    pub(super) unknown_dynlibs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginRuntimeUnitInventoryEntry {
    pub plugin_id: String,
    pub unit: newengine_service_api::RuntimeUnitDescriptor,
}

impl DiscoveryGraph {
    pub fn runtime_unit_inventory(&self) -> Result<Vec<PluginRuntimeUnitInventoryEntry>, String> {
        let mut out = Vec::new();
        for item in &self.items {
            let ScannedDynlibKind::Plugin {
                id, descriptor_v2, ..
            } = &item.kind
            else {
                continue;
            };
            let Some(descriptor) = descriptor_v2.as_ref() else {
                continue;
            };
            let extension = descriptor.extension_json.trim();
            if extension.is_empty() {
                continue;
            }
            let value: serde_json::Value = serde_json::from_str(extension).map_err(|error| {
                format!(
                    "plugin '{}' runtime-unit extension_json is invalid: {}",
                    id, error
                )
            })?;
            let Some(runtime_units) = value.get("runtime_units") else {
                continue;
            };
            let units: Vec<newengine_service_api::RuntimeUnitDescriptor> =
                serde_json::from_value(runtime_units.clone()).map_err(|error| {
                    format!(
                        "plugin '{}' runtime_units metadata is invalid: {}",
                        id, error
                    )
                })?;
            for unit in units {
                out.push(PluginRuntimeUnitInventoryEntry {
                    plugin_id: id.clone(),
                    unit,
                });
            }
        }
        Ok(out)
    }
}

#[derive(Copy, Clone)]
pub(super) enum LoadPhaseFilter {
    All,
    BootstrapOnly,
    /// Runtime/engine load stage. Includes bootstrap plugins when early
    /// bootstrap preloading is deferred, but still excludes platform runtimes.
    BootstrapAndEngine,
}

impl LoadPhaseFilter {
    #[inline]
    pub(super) fn allows(self, phase: PluginBootstrapPhase) -> bool {
        match self {
            Self::All => true,
            Self::BootstrapOnly => matches!(phase, PluginBootstrapPhase::Bootstrap),
            Self::BootstrapAndEngine => matches!(
                phase,
                PluginBootstrapPhase::Bootstrap | PluginBootstrapPhase::Engine
            ),
        }
    }

    #[inline]
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::BootstrapOnly => "bootstrap-only",
            Self::BootstrapAndEngine => "bootstrap+engine",
        }
    }
}

#[inline]
pub(super) fn phase_name(phase: PluginBootstrapPhase) -> &'static str {
    match phase {
        PluginBootstrapPhase::Bootstrap => "bootstrap",
        PluginBootstrapPhase::Platform => "platform",
        PluginBootstrapPhase::Engine => "engine",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use newengine_plugin_api::{PluginBootstrapPhase, PluginDescriptorV2, PluginKind};

    #[test]
    fn plugin_extension_json_contributes_runtime_unit_inventory() {
        let descriptor_v2 = PluginDescriptorV2 {
            id: "plugin.example".into(),
            name: "Example".into(),
            version: "1.0.0".into(),
            kind: PluginKind::Runtime,
            capabilities: Vec::new().into(),
            extension_json: serde_json::json!({
                "runtime_units": [{
                    "id": "plugin.runtime.example",
                    "version": 2,
                    "kind": "product_extension",
                    "provides": ["example.runtime"],
                    "requires": ["scene.backend"],
                    "tags": ["plugin", "headless"]
                }]
            })
            .to_string()
            .into(),
        };
        let graph = DiscoveryGraph {
            dir: PathBuf::from("plugins"),
            entries_total: 1,
            skipped_non_dynlib: 0,
            items: vec![ScannedDynlib {
                path: PathBuf::from("plugins/example.dll"),
                file_name: "example.dll".to_owned(),
                discovery_manifest: None,
                kind: ScannedDynlibKind::Plugin {
                    id: "plugin.example".to_owned(),
                    version: "1.0.0".to_owned(),
                    phase: PluginBootstrapPhase::Engine,
                    descriptor_kind: Some(PluginKind::Runtime),
                    declared_capabilities: Some(0),
                    descriptor: None,
                    descriptor_v2: Some(descriptor_v2),
                    service_gateways: Vec::new(),
                    backend_priority: 0,
                },
            }],
            scan_errors: Vec::new(),
            platform_runtime_count: 0,
            bootstrap_total: 0,
            engine_total: 1,
            unknown_dynlibs: Vec::new(),
        };

        let inventory = graph
            .runtime_unit_inventory()
            .expect("runtime-unit inventory");
        assert_eq!(inventory.len(), 1);
        assert_eq!(inventory[0].plugin_id, "plugin.example");
        assert_eq!(inventory[0].unit.id, "plugin.runtime.example");
        assert_eq!(inventory[0].unit.version, 2);
        assert_eq!(inventory[0].unit.provides, vec!["example.runtime"]);
    }
}
