use std::collections::BTreeSet;

use newengine_core::{Engine, EngineError, EngineResult, StartupConfig};
use newengine_service_api::{EngineCompositionSpec, RuntimeUnitRequirementDescriptor};

use super::super::types::{RuntimeHostRuntimeUnitRegistration, RuntimeUnitCompositionReport};
use super::catalog::{build_runtime_unit_catalog, RuntimeUnitMaterializer};
use super::solver::select_runtime_unit_keys;

pub(in super::super) fn materialize_runtime_units(
    engine: &mut Engine<()>,
    startup: &StartupConfig,
    composition: EngineCompositionSpec,
    distribution_registrations: &'static [RuntimeHostRuntimeUnitRegistration],
    profile_registrations: &'static [RuntimeHostRuntimeUnitRegistration],
    extra_runtime_unit_requirements: &[RuntimeUnitRequirementDescriptor],
    include_plugin_inventory: bool,
) -> EngineResult<RuntimeUnitCompositionReport> {
    let plugin_units = if include_plugin_inventory {
        engine.scan_plugin_runtime_unit_inventory()?
    } else {
        Vec::new()
    };
    let catalog = build_runtime_unit_catalog(
        composition,
        distribution_registrations,
        profile_registrations,
        plugin_units,
    )
    .map_err(EngineError::Other)?;
    let descriptors = catalog.descriptors();
    let selected =
        select_runtime_unit_keys(composition, &descriptors, extra_runtime_unit_requirements)
            .map_err(EngineError::Other)?;
    let mut report = RuntimeUnitCompositionReport {
        selected_units: selected.clone(),
        provided_capabilities: BTreeSet::new(),
    };

    for candidate_key in selected {
        let registration = catalog.registration(&candidate_key).ok_or_else(|| {
            EngineError::Other(format!(
                "composition runtime-unit '{}' was selected but is missing from merged inventory",
                candidate_key
            ))
        })?;
        report
            .provided_capabilities
            .extend(registration.descriptor.provides.iter().cloned());
        let lifecycle = match registration.materializer.as_ref() {
            Some(RuntimeUnitMaterializer::Static(factory)) => {
                let module = factory(engine, startup)?;
                let lifecycle = module.is_some();
                if let Some(module) = module {
                    engine.register_module(module)?;
                }
                lifecycle
            }
            Some(RuntimeUnitMaterializer::Plugin { plugin_id }) => {
                engine.require_plugin_id(plugin_id.clone())?;
                false
            }
            None => {
                return Err(EngineError::Other(format!(
                    "runtime-unit '{}' version={} was selected from inventory but has no materializer; supply a profile/game RuntimeHostRuntimeUnitRegistration or plugin runtime-unit metadata",
                    registration.descriptor.id, registration.descriptor.version
                )));
            }
        };
        newengine_ulog_api::ulog::info!(
            "composition runtime unit selected composition='{}' unit='{}' version={} kind={:?} sources='{}' provides='{}' requires='{}' lifecycle_module={}",
            composition.id,
            registration.descriptor.id,
            registration.descriptor.version,
            registration.descriptor.kind,
            registration.sources.iter().cloned().collect::<Vec<_>>().join(","),
            registration.descriptor.provides.join(","),
            registration.descriptor.requires.join(","),
            lifecycle,
        );
    }
    Ok(report)
}
