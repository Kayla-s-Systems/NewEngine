use std::collections::{BTreeMap, BTreeSet};

use newengine_service_api::{EngineCompositionSpec, EngineRuntimeUnitSpec, RuntimeUnitDescriptor};

use super::super::types::{RuntimeHostRuntimeUnitRegistration, RuntimeUnitFactory};

#[derive(Clone)]
pub(super) enum RuntimeUnitMaterializer {
    Static(RuntimeUnitFactory),
    Plugin { plugin_id: String },
}

#[derive(Clone)]
pub(super) struct RuntimeUnitRegistration {
    pub(super) descriptor: RuntimeUnitDescriptor,
    pub(super) materializer: Option<RuntimeUnitMaterializer>,
    pub(super) sources: BTreeSet<String>,
}

#[derive(Default)]
pub(super) struct RuntimeUnitCatalog {
    pub(super) registrations: BTreeMap<String, RuntimeUnitRegistration>,
}

impl RuntimeUnitCatalog {
    pub(super) fn register_descriptor(
        &mut self,
        descriptor: RuntimeUnitDescriptor,
        source: impl Into<String>,
        materializer: Option<RuntimeUnitMaterializer>,
    ) -> Result<(), String> {
        if descriptor.id.trim().is_empty() {
            return Err("runtime-unit descriptor id must not be empty".to_owned());
        }
        let key = descriptor.candidate_key();
        let source = source.into();
        match self.registrations.get_mut(&key) {
            Some(existing) => {
                if existing.descriptor != descriptor {
                    return Err(format!(
                        "runtime-unit descriptor conflict key='{}' sources='{}' vs '{}'",
                        key,
                        existing
                            .sources
                            .iter()
                            .cloned()
                            .collect::<Vec<_>>()
                            .join(","),
                        source,
                    ));
                }
                match (&existing.materializer, materializer) {
                    (None, Some(materializer)) => existing.materializer = Some(materializer),
                    (
                        Some(RuntimeUnitMaterializer::Plugin {
                            plugin_id: existing_id,
                        }),
                        Some(RuntimeUnitMaterializer::Plugin { plugin_id }),
                    ) if *existing_id == plugin_id => {}
                    (Some(_), Some(_)) => {
                        return Err(format!(
                            "runtime-unit '{}' version={} has multiple materializers",
                            descriptor.id, descriptor.version
                        ));
                    }
                    _ => {}
                }
                existing.sources.insert(source);
            }
            None => {
                let mut sources = BTreeSet::new();
                sources.insert(source);
                self.registrations.insert(
                    key,
                    RuntimeUnitRegistration {
                        descriptor,
                        materializer,
                        sources,
                    },
                );
            }
        }
        Ok(())
    }

    #[inline]
    pub(super) fn register_static(
        &mut self,
        spec: EngineRuntimeUnitSpec,
        source: &'static str,
        factory: RuntimeUnitFactory,
    ) -> Result<(), String> {
        self.register_descriptor(
            RuntimeUnitDescriptor::from_static(spec),
            source,
            Some(RuntimeUnitMaterializer::Static(factory)),
        )
    }

    #[inline]
    pub(super) fn descriptors(&self) -> Vec<RuntimeUnitDescriptor> {
        self.registrations
            .values()
            .map(|registration| registration.descriptor.clone())
            .collect()
    }

    #[inline]
    pub(super) fn registration(&self, candidate_key: &str) -> Option<&RuntimeUnitRegistration> {
        self.registrations.get(candidate_key)
    }
}

pub(super) fn distribution_runtime_unit_catalog(
    distribution_registrations: &'static [RuntimeHostRuntimeUnitRegistration],
) -> Result<RuntimeUnitCatalog, String> {
    let mut catalog = RuntimeUnitCatalog::default();

    for registration in distribution_registrations {
        catalog.register_static(
            registration.spec,
            "distribution:profile-selected-static",
            registration.factory,
        )?;
    }

    Ok(catalog)
}

pub(super) fn build_runtime_unit_catalog(
    composition: EngineCompositionSpec,
    distribution_registrations: &'static [RuntimeHostRuntimeUnitRegistration],
    profile_registrations: &'static [RuntimeHostRuntimeUnitRegistration],
    plugin_units: Vec<newengine_plugin_host::PluginRuntimeUnitInventoryEntry>,
) -> Result<RuntimeUnitCatalog, String> {
    let mut catalog = distribution_runtime_unit_catalog(distribution_registrations)?;

    // EngineCompositionSpec.runtime_units is inventory, not an imperative activation list.
    for spec in composition.runtime_units {
        catalog.register_descriptor(
            RuntimeUnitDescriptor::from_static(*spec),
            format!("composition:{}", composition.id),
            None,
        )?;
    }

    // Profile/game code may bind factories for inventory entries not shipped by the distribution.
    for registration in profile_registrations {
        catalog.register_static(
            registration.spec,
            "profile-or-game:static-factory",
            registration.factory,
        )?;
    }

    // Plugin descriptors contribute dynamic inventory. Selecting one marks its plugin required;
    // actual DLL initialization remains owned by the normal plugin loading phase.
    for entry in plugin_units {
        let plugin_id = entry.plugin_id;
        catalog.register_descriptor(
            entry.unit,
            format!("plugin:{plugin_id}"),
            Some(RuntimeUnitMaterializer::Plugin { plugin_id }),
        )?;
    }

    Ok(catalog)
}
