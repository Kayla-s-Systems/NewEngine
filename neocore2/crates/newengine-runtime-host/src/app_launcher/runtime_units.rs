use std::collections::BTreeSet;

use newengine_core::{Engine, EngineError, EngineResult, Module, StartupConfig};
use newengine_service_api::{
    CapabilityMatrix, CapabilityRequirement, CompositionCandidate, CompositionSolver,
    CompositionSolverInput, EngineCompositionSpec, EngineRuntimeUnitSpec, RequirementStrength,
};

type RuntimeUnitFactory = fn(&StartupConfig) -> Box<dyn Module<()>>;

#[derive(Clone, Copy)]
struct RuntimeUnitRegistration {
    spec: EngineRuntimeUnitSpec,
    factory: RuntimeUnitFactory,
}

#[derive(Default)]
struct RuntimeUnitCatalog {
    registrations: Vec<RuntimeUnitRegistration>,
}

impl RuntimeUnitCatalog {
    #[inline]
    fn new() -> Self {
        Self::default()
    }

    fn register(
        &mut self,
        spec: EngineRuntimeUnitSpec,
        factory: RuntimeUnitFactory,
    ) -> Result<(), String> {
        if spec.id.trim().is_empty() {
            return Err("runtime-unit descriptor id must not be empty".to_owned());
        }
        if self
            .registrations
            .iter()
            .any(|registration| registration.spec.id == spec.id)
        {
            return Err(format!("duplicate runtime-unit id '{}'", spec.id));
        }
        self.registrations
            .push(RuntimeUnitRegistration { spec, factory });
        self.registrations.sort_by(|a, b| a.spec.id.cmp(b.spec.id));
        Ok(())
    }

    fn specs(&self) -> Vec<EngineRuntimeUnitSpec> {
        self.registrations
            .iter()
            .map(|registration| registration.spec)
            .collect()
    }

    #[inline]
    fn registration(&self, unit_id: &str) -> Option<&RuntimeUnitRegistration> {
        self.registrations
            .binary_search_by(|registration| registration.spec.id.cmp(unit_id))
            .ok()
            .map(|index| &self.registrations[index])
    }
}

#[cfg(feature = "standard-backend-adapters")]
fn render_runtime_unit(startup: &StartupConfig) -> Box<dyn Module<()>> {
    Box::new(
        newengine_render_runtime_adapter::RenderBackendRuntimeModule::new(
            startup.modules_dir.clone(),
        ),
    )
}

#[cfg(feature = "standard-backend-adapters")]
fn physics_runtime_unit(startup: &StartupConfig) -> Box<dyn Module<()>> {
    Box::new(
        newengine_physics_runtime_adapter::PhysicsBackendRuntimeModule::new(
            startup.modules_dir.clone(),
        ),
    )
}

fn standard_runtime_unit_catalog() -> Result<RuntimeUnitCatalog, String> {
    let mut catalog = RuntimeUnitCatalog::new();
    #[cfg(feature = "standard-backend-adapters")]
    {
        catalog.register(
            newengine_render_runtime_adapter::RENDER_RUNTIME_UNIT_SPEC,
            render_runtime_unit,
        )?;
        catalog.register(
            newengine_physics_runtime_adapter::PHYSICS_RUNTIME_UNIT_SPEC,
            physics_runtime_unit,
        )?;
    }
    Ok(catalog)
}

fn activation_requirement(requirement: &CapabilityRequirement) -> CapabilityRequirement {
    match requirement.strength {
        RequirementStrength::Required => CapabilityRequirement::required(requirement.capability),
        RequirementStrength::Preferred => CapabilityRequirement::preferred(requirement.capability),
        RequirementStrength::Optional => CapabilityRequirement::optional(requirement.capability),
    }
}

/// Selects implementation units through the same deterministic CompositionSolver
/// used for provider routes.
///
/// `EngineRuntimeUnitSpec::requires` is the logical capability that activates the
/// implementation unit (`render.backend`, `physics.backend`, ...). `provides` is the
/// host-local API produced by the bridge (`engine.runtime.render-api`, ...). This keeps
/// provider selection and adapter materialization distinct: a runtime adapter never
/// masquerades as a backend provider.
fn select_runtime_unit_ids(
    composition: EngineCompositionSpec,
    specs: &[EngineRuntimeUnitSpec],
) -> Result<Vec<String>, String> {
    let bridged_capabilities = specs
        .iter()
        .flat_map(|spec| spec.requires.iter().copied())
        .collect::<BTreeSet<_>>();

    let activation_requirements = composition
        .requirements
        .iter()
        .filter(|requirement| bridged_capabilities.contains(requirement.capability.as_str()))
        .map(activation_requirement)
        .collect::<Vec<_>>();

    if activation_requirements.is_empty() {
        return Ok(Vec::new());
    }

    let matrix = CapabilityMatrix::from_specs(composition.id, &activation_requirements);
    let mut candidates = Vec::new();
    for spec in specs {
        for required_capability in spec.requires {
            let Some(requirement) = composition
                .requirements
                .iter()
                .find(|requirement| requirement.capability.as_str() == *required_capability)
            else {
                continue;
            };
            candidates.push(
                CompositionCandidate::new(
                    requirement.capability.gateway_id(),
                    spec.id,
                    "engine.runtime-host.runtime-unit-catalog",
                    spec.version.min(i32::MAX as u32) as i32,
                    0,
                    0,
                )
                .with_capability(*required_capability)
                .with_tags(spec.tags.iter().copied()),
            );
        }
    }

    let plan = CompositionSolver::resolve_input(CompositionSolverInput {
        candidates,
        capability_matrix: matrix,
    });
    plan.validate_required()?;

    let mut selected = BTreeSet::new();
    for gateway_id in plan.gateway_ids() {
        for unit in plan.selected_all(&gateway_id) {
            selected.insert(unit.candidate_id.clone());
        }
    }
    Ok(selected.into_iter().collect())
}

pub(super) fn materialize_declared_runtime_units(
    engine: &mut Engine<()>,
    startup: &StartupConfig,
    composition: EngineCompositionSpec,
) -> EngineResult<()> {
    let catalog = standard_runtime_unit_catalog().map_err(EngineError::Other)?;
    let specs = catalog.specs();
    let selected = select_runtime_unit_ids(composition, &specs).map_err(EngineError::Other)?;

    for unit_id in selected {
        let registration = catalog.registration(&unit_id).ok_or_else(|| {
            EngineError::Other(format!(
                "composition runtime-unit '{}' was selected but has no factory registration",
                unit_id
            ))
        })?;
        engine.register_module((registration.factory)(startup))?;
        newengine_ulog_api::ulog::info!(
            "composition runtime unit materialized composition='{}' unit='{}' kind={:?} activates_on='{}' provides='{}'",
            composition.id,
            registration.spec.id,
            registration.spec.kind,
            registration.spec.requires.join(","),
            registration.spec.provides.join(","),
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use newengine_service_api::{CapabilityId, EngineRuntimeUnitKind};

    const RENDER: CapabilityId = CapabilityId::new("render.backend", "engine.render", "render");
    const PHYSICS: CapabilityId = CapabilityId::new("physics.backend", "engine.physics", "physics");
    const UI: CapabilityId = CapabilityId::new("ui.backend", "engine.ui", "ui");

    const REQUIREMENTS: &[CapabilityRequirement] = &[
        CapabilityRequirement::required(RENDER),
        CapabilityRequirement::required(PHYSICS),
        CapabilityRequirement::required(UI),
    ];
    const RENDER_V1: EngineRuntimeUnitSpec = EngineRuntimeUnitSpec::new(
        "render.bridge.v1",
        1,
        EngineRuntimeUnitKind::Adapter,
        &["engine.runtime.render-api"],
        &["render.backend"],
        &["backend-neutral"],
    );
    const RENDER_V2: EngineRuntimeUnitSpec = EngineRuntimeUnitSpec::new(
        "render.bridge.v2",
        2,
        EngineRuntimeUnitKind::Adapter,
        &["engine.runtime.render-api"],
        &["render.backend"],
        &["backend-neutral"],
    );
    const PHYSICS_V1: EngineRuntimeUnitSpec = EngineRuntimeUnitSpec::new(
        "physics.bridge.v1",
        1,
        EngineRuntimeUnitKind::Adapter,
        &["engine.runtime.physics-api"],
        &["physics.backend"],
        &["backend-neutral"],
    );

    fn noop_runtime_unit(_startup: &StartupConfig) -> Box<dyn Module<()>> {
        struct Noop;
        impl Module<()> for Noop {
            fn id(&self) -> &'static str {
                "test.runtime-unit.noop"
            }
        }
        Box::new(Noop)
    }

    #[test]
    fn solver_selects_runtime_bridges_without_product_module_names() {
        let composition = EngineCompositionSpec::new("test.product", REQUIREMENTS);
        let selected = select_runtime_unit_ids(composition, &[RENDER_V1, PHYSICS_V1, RENDER_V2])
            .expect("runtime-unit composition");
        assert_eq!(
            selected,
            vec![
                "physics.bridge.v1".to_owned(),
                "render.bridge.v2".to_owned()
            ]
        );
    }

    #[test]
    fn capabilities_without_local_bridge_do_not_become_false_failures() {
        let composition = EngineCompositionSpec::new("test.product", REQUIREMENTS);
        let selected = select_runtime_unit_ids(composition, &[RENDER_V1, PHYSICS_V1])
            .expect("runtime-unit composition");
        assert_eq!(selected.len(), 2);
        assert!(!selected.iter().any(|id| id.contains("ui")));
    }

    #[test]
    fn catalog_rejects_duplicate_unit_ids() {
        let mut catalog = RuntimeUnitCatalog::new();
        catalog
            .register(RENDER_V1, noop_runtime_unit)
            .expect("first registration");
        let error = catalog
            .register(RENDER_V1, noop_runtime_unit)
            .expect_err("duplicate runtime-unit id must fail");
        assert!(error.contains("duplicate runtime-unit id"));
    }

    #[cfg(feature = "standard-backend-adapters")]
    #[test]
    fn standard_catalog_contains_backend_neutral_render_and_physics_bridges() {
        let catalog = standard_runtime_unit_catalog().expect("standard runtime-unit catalog");
        let specs = catalog.specs();
        assert!(specs.iter().any(|spec| {
            spec.id == "engine.runtime-adapter.render" && spec.requires.contains(&"render.backend")
        }));
        assert!(specs.iter().any(|spec| {
            spec.id == "engine.runtime-adapter.physics"
                && spec.requires.contains(&"physics.backend")
        }));
    }
}
