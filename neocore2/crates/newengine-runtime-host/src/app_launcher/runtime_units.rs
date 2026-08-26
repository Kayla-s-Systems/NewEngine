use std::collections::{BTreeMap, BTreeSet};

use newengine_core::{Engine, EngineError, EngineResult, Module, StartupConfig};
use newengine_service_api::{
    CapabilityMatrix, CompositionCandidate, CompositionRequirement, CompositionSolver,
    CompositionSolverInput, EngineCompositionSpec, EngineRuntimeUnitSpec, RuntimeUnitDescriptor,
    RuntimeUnitRequirementDescriptor, RuntimeUnitRequirementSpec,
};

use super::types::{RuntimeHostRuntimeUnitRegistration, RuntimeUnitFactory};

#[derive(Clone)]
enum RuntimeUnitMaterializer {
    Static(RuntimeUnitFactory),
    Plugin { plugin_id: String },
}

#[derive(Clone)]
struct RuntimeUnitRegistration {
    descriptor: RuntimeUnitDescriptor,
    materializer: Option<RuntimeUnitMaterializer>,
    sources: BTreeSet<String>,
}

#[derive(Default)]
struct RuntimeUnitCatalog {
    registrations: BTreeMap<String, RuntimeUnitRegistration>,
}

impl RuntimeUnitCatalog {
    fn register_descriptor(
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
    fn register_static(
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
    fn descriptors(&self) -> Vec<RuntimeUnitDescriptor> {
        self.registrations
            .values()
            .map(|registration| registration.descriptor.clone())
            .collect()
    }

    #[inline]
    fn registration(&self, candidate_key: &str) -> Option<&RuntimeUnitRegistration> {
        self.registrations.get(candidate_key)
    }
}

#[cfg(feature = "standard-backend-adapters")]
fn render_runtime_unit(
    _engine: &mut Engine<()>,
    startup: &StartupConfig,
) -> EngineResult<Option<Box<dyn Module<()>>>> {
    Ok(Some(Box::new(
        newengine_render_runtime_adapter::RenderBackendRuntimeModule::new(
            startup.modules_dir.clone(),
        ),
    )))
}

#[cfg(feature = "standard-backend-adapters")]
fn physics_runtime_unit(
    _engine: &mut Engine<()>,
    startup: &StartupConfig,
) -> EngineResult<Option<Box<dyn Module<()>>>> {
    Ok(Some(Box::new(
        newengine_physics_runtime_adapter::PhysicsBackendRuntimeModule::new(
            startup.modules_dir.clone(),
        ),
    )))
}

fn distribution_runtime_unit_catalog() -> Result<RuntimeUnitCatalog, String> {
    let mut catalog = RuntimeUnitCatalog::default();

    #[cfg(feature = "standard-backend-adapters")]
    {
        catalog.register_static(
            newengine_render_runtime_adapter::RENDER_RUNTIME_UNIT_SPEC,
            "distribution:render-adapter",
            render_runtime_unit,
        )?;
        catalog.register_static(
            newengine_physics_runtime_adapter::PHYSICS_RUNTIME_UNIT_SPEC,
            "distribution:physics-adapter",
            physics_runtime_unit,
        )?;
    }

    #[cfg(feature = "full-runtime")]
    for registration in newengine_runtime_units::STATIC_RUNTIME_UNIT_REGISTRATIONS {
        catalog.register_static(
            registration.spec,
            "distribution:first-party-static",
            registration.factory,
        )?;
    }

    Ok(catalog)
}

fn build_runtime_unit_catalog(
    composition: EngineCompositionSpec,
    profile_registrations: &'static [RuntimeHostRuntimeUnitRegistration],
    plugin_units: Vec<newengine_plugin_host::PluginRuntimeUnitInventoryEntry>,
) -> Result<RuntimeUnitCatalog, String> {
    let mut catalog = distribution_runtime_unit_catalog()?;

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

fn runtime_unit_is_forbidden(
    composition: EngineCompositionSpec,
    descriptor: &RuntimeUnitDescriptor,
) -> bool {
    composition.forbidden_tags.iter().any(|tag| {
        descriptor
            .tags
            .iter()
            .any(|candidate| candidate == tag.as_str())
    })
}

fn runtime_unit_requirement_static(
    composition_id: &str,
    spec: &RuntimeUnitRequirementSpec,
) -> CompositionRequirement {
    runtime_unit_requirement_owned(
        composition_id,
        &RuntimeUnitRequirementDescriptor::from_static(*spec),
    )
}

fn runtime_unit_requirement_owned(
    composition_id: &str,
    spec: &RuntimeUnitRequirementDescriptor,
) -> CompositionRequirement {
    let strength = spec.strength();
    let min_cardinality = spec.cardinality.min(strength);
    CompositionRequirement {
        capability_id: spec.capability.trim().to_owned(),
        gateway_id: format!("engine.runtime-unit.capability:{}", spec.capability.trim()),
        service_kind: "runtime-unit".to_owned(),
        level: strength,
        min_capability_version: 0,
        max_capability_version: None,
        contract_id: None,
        min_contract_version: 0,
        max_contract_version: None,
        required_tags: spec
            .required_tags
            .iter()
            .map(|tag| tag.trim().to_owned())
            .collect(),
        preferred_tags: spec
            .preferred_tags
            .iter()
            .map(|tag| tag.trim().to_owned())
            .collect(),
        conflict_tags: spec
            .forbidden_tags
            .iter()
            .map(|tag| tag.trim().to_owned())
            .collect(),
        fallback_provider_ids: Vec::new(),
        min_cardinality,
        max_cardinality: spec.cardinality.max().max(min_cardinality),
        declared_by: format!("{composition_id}:runtime-units"),
    }
}

fn solve_candidates(
    composition: EngineCompositionSpec,
    descriptors: &[RuntimeUnitDescriptor],
    capability_source: fn(&RuntimeUnitDescriptor) -> &[String],
    required_tag: Option<&str>,
    include_runtime_unit_roots: bool,
    extra_runtime_unit_requirements: &[RuntimeUnitRequirementDescriptor],
) -> Result<BTreeSet<String>, String> {
    let advertised = descriptors
        .iter()
        .filter(|descriptor| {
            required_tag.is_none_or(|tag| descriptor.tags.iter().any(|candidate| candidate == tag))
        })
        .filter(|descriptor| !runtime_unit_is_forbidden(composition, descriptor))
        .flat_map(|descriptor| capability_source(descriptor).iter().map(String::as_str))
        .collect::<BTreeSet<_>>();

    let mut activation_requirements = composition
        .requirements
        .iter()
        .filter(|requirement| advertised.contains(requirement.capability.as_str()))
        .map(|requirement| CompositionRequirement::from_spec(requirement, composition.id))
        .collect::<Vec<_>>();
    if include_runtime_unit_roots {
        activation_requirements.extend(
            composition
                .runtime_unit_requirements
                .iter()
                .map(|requirement| runtime_unit_requirement_static(composition.id, requirement)),
        );
        activation_requirements.extend(
            extra_runtime_unit_requirements
                .iter()
                .map(|requirement| runtime_unit_requirement_owned(composition.id, requirement)),
        );
    }
    if activation_requirements.is_empty() {
        return Ok(BTreeSet::new());
    }

    let matrix = CapabilityMatrix::new(activation_requirements)
        .with_preferred_tags(composition.preferred_tags.iter().map(|tag| tag.as_str()))
        .with_forbidden_tags(composition.forbidden_tags.iter().map(|tag| tag.as_str()));
    let requirements = matrix.capability_requirements().to_vec();
    let mut candidates = Vec::new();

    for descriptor in descriptors {
        if required_tag.is_some_and(|tag| !descriptor.tags.iter().any(|candidate| candidate == tag))
            || runtime_unit_is_forbidden(composition, descriptor)
        {
            continue;
        }
        let capabilities = capability_source(descriptor)
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        let mut by_gateway = BTreeMap::<&str, Vec<&str>>::new();
        for requirement in &requirements {
            if capabilities.contains(requirement.capability_id.as_str()) {
                by_gateway
                    .entry(requirement.gateway_id.as_str())
                    .or_default()
                    .push(requirement.capability_id.as_str());
            }
        }
        for (gateway, capabilities) in by_gateway {
            candidates.push(
                CompositionCandidate::new(
                    gateway,
                    descriptor.candidate_key(),
                    "engine.runtime-unit.inventory",
                    descriptor.version.min(i32::MAX as u32) as i32,
                    0,
                    0,
                )
                .with_capabilities(capabilities)
                .with_capability_version(descriptor.version)
                .with_tags(descriptor.tags.iter().cloned()),
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
    Ok(selected)
}

fn select_runtime_unit_keys(
    composition: EngineCompositionSpec,
    descriptors: &[RuntimeUnitDescriptor],
    extra_runtime_unit_requirements: &[RuntimeUnitRequirementDescriptor],
) -> Result<Vec<String>, String> {
    // Service adapters are activated by the external backend capabilities they consume.
    // Provider/module/product units are selected from capabilities they provide. Transitive
    // runtime-unit dependencies are then promoted to ordinary CompositionRequirements and the
    // same CompositionSolver is re-run until the selected unit set reaches a fixed point.
    let adapters = solve_candidates(
        composition,
        descriptors,
        |descriptor| &descriptor.requires,
        Some("service-adapter"),
        false,
        &[],
    )?;

    let external = composition
        .requirements
        .iter()
        .map(|requirement| requirement.capability.as_str())
        .collect::<BTreeSet<_>>();
    let mut dependency_requirements = BTreeMap::<String, RuntimeUnitRequirementDescriptor>::new();
    let mut previous_selected = BTreeSet::<String>::new();

    loop {
        let mut solver_requirements = extra_runtime_unit_requirements.to_vec();
        solver_requirements.extend(dependency_requirements.values().cloned());

        let mut selected = adapters.clone();
        selected.extend(solve_candidates(
            composition,
            descriptors,
            |descriptor| &descriptor.provides,
            None,
            true,
            &solver_requirements,
        )?);

        let selected_descriptors = descriptors
            .iter()
            .filter(|descriptor| selected.contains(&descriptor.candidate_key()))
            .collect::<Vec<_>>();
        let mut discovered_dependency = false;
        for descriptor in selected_descriptors {
            for dependency in &descriptor.requires {
                let dependency = dependency.trim();
                if dependency.is_empty() || external.contains(dependency) {
                    continue;
                }
                if dependency_requirements.contains_key(dependency) {
                    continue;
                }
                dependency_requirements.insert(
                    dependency.to_owned(),
                    RuntimeUnitRequirementDescriptor::required(dependency),
                );
                discovered_dependency = true;
            }
        }

        if !discovered_dependency && selected == previous_selected {
            return topological_runtime_unit_order(descriptors, &selected);
        }
        previous_selected = selected;
    }
}

fn topological_runtime_unit_order(
    descriptors: &[RuntimeUnitDescriptor],
    selected: &BTreeSet<String>,
) -> Result<Vec<String>, String> {
    let by_key = descriptors
        .iter()
        .map(|descriptor| (descriptor.candidate_key(), descriptor))
        .collect::<BTreeMap<_, _>>();
    let mut providers = BTreeMap::<&str, BTreeSet<String>>::new();
    for key in selected {
        let descriptor = by_key.get(key).ok_or_else(|| {
            format!(
                "selected runtime-unit '{}' missing from merged inventory",
                key
            )
        })?;
        for capability in &descriptor.provides {
            providers
                .entry(capability.as_str())
                .or_default()
                .insert(key.clone());
        }
    }

    let mut indegree = selected
        .iter()
        .map(|key| (key.clone(), 0usize))
        .collect::<BTreeMap<_, _>>();
    let mut outgoing = BTreeMap::<String, BTreeSet<String>>::new();
    for key in selected {
        let descriptor = by_key[key];
        for dependency in &descriptor.requires {
            let Some(provider_keys) = providers.get(dependency.as_str()) else {
                continue;
            };
            for provider_key in provider_keys {
                if provider_key == key {
                    continue;
                }
                if outgoing
                    .entry(provider_key.clone())
                    .or_default()
                    .insert(key.clone())
                {
                    *indegree.get_mut(key).expect("selected indegree") += 1;
                }
            }
        }
    }

    let mut ready = indegree
        .iter()
        .filter_map(|(key, degree)| (*degree == 0).then_some(key.clone()))
        .collect::<BTreeSet<_>>();
    let mut ordered = Vec::with_capacity(selected.len());
    while let Some(key) = ready.pop_first() {
        ordered.push(key.clone());
        if let Some(consumers) = outgoing.get(&key) {
            for consumer in consumers {
                let degree = indegree.get_mut(consumer).expect("consumer indegree");
                *degree -= 1;
                if *degree == 0 {
                    ready.insert(consumer.clone());
                }
            }
        }
    }
    if ordered.len() != selected.len() {
        let cyclic = indegree
            .into_iter()
            .filter_map(|(key, degree)| (degree != 0).then_some(key))
            .collect::<Vec<_>>();
        return Err(format!(
            "runtime-unit dependency cycle detected units={}",
            cyclic.join(",")
        ));
    }
    Ok(ordered)
}

pub(super) fn materialize_runtime_units(
    engine: &mut Engine<()>,
    startup: &StartupConfig,
    composition: EngineCompositionSpec,
    profile_registrations: &'static [RuntimeHostRuntimeUnitRegistration],
    extra_runtime_unit_requirements: &[RuntimeUnitRequirementDescriptor],
    include_plugin_inventory: bool,
) -> EngineResult<super::types::RuntimeUnitCompositionReport> {
    let plugin_units = if include_plugin_inventory {
        engine.scan_plugin_runtime_unit_inventory()?
    } else {
        Vec::new()
    };
    let catalog = build_runtime_unit_catalog(composition, profile_registrations, plugin_units)
        .map_err(EngineError::Other)?;
    let descriptors = catalog.descriptors();
    let selected =
        select_runtime_unit_keys(composition, &descriptors, extra_runtime_unit_requirements)
            .map_err(EngineError::Other)?;
    let mut report = super::types::RuntimeUnitCompositionReport {
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

#[cfg(test)]
mod tests {
    use super::*;
    use newengine_service_api::{
        CapabilityId, CapabilityRequirement, EngineRuntimeUnitKind,
        RuntimeUnitRequirementDescriptor, RuntimeUnitRequirementSpec, SystemTag,
    };

    const RENDER: CapabilityId = CapabilityId::new("render.backend", "engine.render", "render");
    const PHYSICS: CapabilityId = CapabilityId::new("physics.backend", "engine.physics", "physics");
    const WORLD: CapabilityId = CapabilityId::new("world.backend", "engine.world", "world");
    const CUSTOM: CapabilityId =
        CapabilityId::new("custom.runtime", "engine.custom", "runtime-unit");
    const ASSET_MANAGER: CapabilityId =
        CapabilityId::new("asset_manager.backend", "engine.assets", "assets");
    const AUDIO: CapabilityId = CapabilityId::new("audio.backend", "engine.audio", "audio");

    const REQUIREMENTS: &[CapabilityRequirement] = &[
        CapabilityRequirement::required(RENDER),
        CapabilityRequirement::required(PHYSICS),
    ];
    const WORLD_REQUIREMENTS: &[CapabilityRequirement] = &[CapabilityRequirement::required(WORLD)];
    const CUSTOM_REQUIREMENTS: &[CapabilityRequirement] =
        &[CapabilityRequirement::required(CUSTOM)];
    const STANDARD_EXTERNAL_REQUIREMENTS: &[CapabilityRequirement] = &[
        CapabilityRequirement::required(ASSET_MANAGER),
        CapabilityRequirement::optional(AUDIO),
    ];
    const RENDER_V1: EngineRuntimeUnitSpec = EngineRuntimeUnitSpec::new(
        "render.bridge.v1",
        1,
        EngineRuntimeUnitKind::Adapter,
        &["engine.runtime.render-api"],
        &["render.backend"],
        &["engine.runtime-unit", "service-adapter"],
    );
    const RENDER_V2: EngineRuntimeUnitSpec = EngineRuntimeUnitSpec::new(
        "render.bridge.v2",
        2,
        EngineRuntimeUnitKind::Adapter,
        &["engine.runtime.render-api"],
        &["render.backend"],
        &["engine.runtime-unit", "service-adapter"],
    );
    const PHYSICS_V1: EngineRuntimeUnitSpec = EngineRuntimeUnitSpec::new(
        "physics.bridge.v1",
        1,
        EngineRuntimeUnitKind::Adapter,
        &["engine.runtime.physics-api"],
        &["physics.backend"],
        &["engine.runtime-unit", "service-adapter"],
    );
    const SCENE_UNIT: EngineRuntimeUnitSpec = EngineRuntimeUnitSpec::new(
        "engine.runtime.scene.test",
        1,
        EngineRuntimeUnitKind::Provider,
        &["scene.backend"],
        &[],
        &["engine.runtime-unit", "static"],
    );
    const WORLD_UNIT: EngineRuntimeUnitSpec = EngineRuntimeUnitSpec::new(
        "engine.runtime.world.test",
        1,
        EngineRuntimeUnitKind::Provider,
        &["world.backend"],
        &["scene.backend"],
        &["engine.runtime-unit", "static"],
    );
    const SCENE_HIGH_VERSION: EngineRuntimeUnitSpec = EngineRuntimeUnitSpec::new(
        "engine.runtime.scene.high-version",
        10,
        EngineRuntimeUnitKind::Provider,
        &["scene.backend"],
        &[],
        &["engine.runtime-unit", "static"],
    );
    const SCENE_PREFERRED: EngineRuntimeUnitSpec = EngineRuntimeUnitSpec::new(
        "engine.runtime.scene.preferred",
        1,
        EngineRuntimeUnitKind::Provider,
        &["scene.backend"],
        &[],
        &["engine.runtime-unit", "static", "runtime.preferred"],
    );
    const CLOCK_UNIT: EngineRuntimeUnitSpec = EngineRuntimeUnitSpec::new(
        "engine.runtime.clock.test",
        1,
        EngineRuntimeUnitKind::Provider,
        &["clock.backend"],
        &[],
        &["engine.runtime-unit", "static"],
    );
    const SCENE_TRANSITIVE_UNIT: EngineRuntimeUnitSpec = EngineRuntimeUnitSpec::new(
        "engine.runtime.scene.transitive",
        1,
        EngineRuntimeUnitKind::Provider,
        &["scene.backend"],
        &["clock.backend"],
        &["engine.runtime-unit", "static"],
    );
    const PREFERRED_RUNTIME: SystemTag = SystemTag::new("runtime.preferred");
    const CUSTOM_UNIT: EngineRuntimeUnitSpec = EngineRuntimeUnitSpec::new(
        "profile.runtime.custom",
        1,
        EngineRuntimeUnitKind::ProductExtension,
        &["custom.runtime"],
        &[],
        &["engine.runtime-unit", "profile"],
    );
    const PROFILE_INVENTORY: &[EngineRuntimeUnitSpec] = &[CUSTOM_UNIT];

    fn noop_runtime_unit(
        _engine: &mut Engine<()>,
        _startup: &StartupConfig,
    ) -> EngineResult<Option<Box<dyn Module<()>>>> {
        Ok(None)
    }

    #[test]
    fn solver_selects_implicit_runtime_bridges() {
        let composition = EngineCompositionSpec::new("test.product", REQUIREMENTS);
        let descriptors = [RENDER_V1, PHYSICS_V1, RENDER_V2]
            .into_iter()
            .map(RuntimeUnitDescriptor::from_static)
            .collect::<Vec<_>>();
        let selected = select_runtime_unit_keys(composition, &descriptors, &[])
            .expect("runtime-unit composition");
        assert!(selected.contains(&"render.bridge.v2@2".to_owned()));
        assert!(selected.contains(&"physics.bridge.v1@1".to_owned()));
        assert!(!selected.contains(&"render.bridge.v1@1".to_owned()));
    }

    #[test]
    fn profile_runtime_units_are_inventory_candidates_not_implicit_roots() {
        let no_requirement =
            EngineCompositionSpec::new("test.profile", &[]).with_runtime_units(PROFILE_INVENTORY);
        let descriptors = PROFILE_INVENTORY
            .iter()
            .copied()
            .map(RuntimeUnitDescriptor::from_static)
            .collect::<Vec<_>>();
        assert!(select_runtime_unit_keys(no_requirement, &descriptors, &[])
            .unwrap()
            .is_empty());

        let required = EngineCompositionSpec::new("test.profile", CUSTOM_REQUIREMENTS)
            .with_runtime_units(PROFILE_INVENTORY);
        assert_eq!(
            select_runtime_unit_keys(required, &descriptors, &[]).unwrap(),
            vec!["profile.runtime.custom@1".to_owned()]
        );
    }

    #[test]
    fn runtime_unit_only_requirement_selects_from_profile_inventory() {
        const ROOTS: &[RuntimeUnitRequirementSpec] =
            &[RuntimeUnitRequirementSpec::required("custom.runtime")];
        let composition = EngineCompositionSpec::new("test.profile-roots", &[])
            .with_runtime_units(PROFILE_INVENTORY)
            .with_runtime_unit_requirements(ROOTS);
        let descriptors = PROFILE_INVENTORY
            .iter()
            .copied()
            .map(RuntimeUnitDescriptor::from_static)
            .collect::<Vec<_>>();
        assert_eq!(
            select_runtime_unit_keys(composition, &descriptors, &[]).unwrap(),
            vec!["profile.runtime.custom@1".to_owned()]
        );
    }

    #[test]
    fn missing_runtime_unit_root_is_a_hard_error() {
        const ROOTS: &[RuntimeUnitRequirementSpec] =
            &[RuntimeUnitRequirementSpec::required("missing.runtime")];
        let composition = EngineCompositionSpec::new("test.missing-root", &[])
            .with_runtime_unit_requirements(ROOTS);
        let error = select_runtime_unit_keys(composition, &[], &[])
            .expect_err("missing runtime-unit root must fail composition");
        assert!(error.contains("missing.runtime"));
    }

    #[test]
    fn dependency_closure_uses_combined_inventory() {
        let composition = EngineCompositionSpec::new("test.static", WORLD_REQUIREMENTS)
            .with_runtime_units(&[WORLD_UNIT, SCENE_UNIT]);
        let descriptors = [WORLD_UNIT, SCENE_UNIT]
            .into_iter()
            .map(RuntimeUnitDescriptor::from_static)
            .collect::<Vec<_>>();
        let selected = select_runtime_unit_keys(composition, &descriptors, &[]).unwrap();
        assert!(selected.contains(&"engine.runtime.world.test@1".to_owned()));
        assert!(selected.contains(&"engine.runtime.scene.test@1".to_owned()));
    }

    #[test]
    fn dependency_closure_provider_choice_is_owned_by_composition_solver() {
        let composition =
            EngineCompositionSpec::new("test.fixed-point-preference", WORLD_REQUIREMENTS)
                .with_runtime_units(&[WORLD_UNIT, SCENE_HIGH_VERSION, SCENE_PREFERRED])
                .with_preferred_tags(&[PREFERRED_RUNTIME]);
        let descriptors = [WORLD_UNIT, SCENE_HIGH_VERSION, SCENE_PREFERRED]
            .into_iter()
            .map(RuntimeUnitDescriptor::from_static)
            .collect::<Vec<_>>();

        let selected = select_runtime_unit_keys(composition, &descriptors, &[])
            .expect("dependency provider must resolve through shared solver");

        assert!(selected.contains(&"engine.runtime.world.test@1".to_owned()));
        assert!(selected.contains(&"engine.runtime.scene.preferred@1".to_owned()));
        assert!(!selected.contains(&"engine.runtime.scene.high-version@10".to_owned()));
    }

    #[test]
    fn dependency_closure_reaches_transitive_fixed_point() {
        let composition =
            EngineCompositionSpec::new("test.fixed-point-transitive", WORLD_REQUIREMENTS)
                .with_runtime_units(&[WORLD_UNIT, SCENE_TRANSITIVE_UNIT, CLOCK_UNIT]);
        let descriptors = [WORLD_UNIT, SCENE_TRANSITIVE_UNIT, CLOCK_UNIT]
            .into_iter()
            .map(RuntimeUnitDescriptor::from_static)
            .collect::<Vec<_>>();

        let selected = select_runtime_unit_keys(composition, &descriptors, &[])
            .expect("transitive dependency closure");

        assert_eq!(
            selected,
            vec![
                "engine.runtime.clock.test@1".to_owned(),
                "engine.runtime.scene.transitive@1".to_owned(),
                "engine.runtime.world.test@1".to_owned(),
            ]
        );
    }

    #[test]
    fn catalog_merges_distribution_descriptor_and_profile_inventory() {
        let mut catalog = RuntimeUnitCatalog::default();
        catalog
            .register_static(CUSTOM_UNIT, "distribution", noop_runtime_unit)
            .unwrap();
        catalog
            .register_descriptor(
                RuntimeUnitDescriptor::from_static(CUSTOM_UNIT),
                "profile",
                None,
            )
            .unwrap();
        let entry = catalog.registration("profile.runtime.custom@1").unwrap();
        assert_eq!(entry.sources.len(), 2);
        assert!(matches!(
            entry.materializer,
            Some(RuntimeUnitMaterializer::Static(_))
        ));
    }

    #[test]
    fn catalog_allows_multiple_versions_of_same_unit_id() {
        let v2 = EngineRuntimeUnitSpec::new(
            CUSTOM_UNIT.id,
            2,
            CUSTOM_UNIT.kind,
            CUSTOM_UNIT.provides,
            CUSTOM_UNIT.requires,
            CUSTOM_UNIT.tags,
        );
        let mut catalog = RuntimeUnitCatalog::default();
        catalog
            .register_static(CUSTOM_UNIT, "v1", noop_runtime_unit)
            .unwrap();
        catalog
            .register_static(v2, "v2", noop_runtime_unit)
            .unwrap();
        assert_eq!(catalog.registrations.len(), 2);
    }

    #[cfg(feature = "full-runtime")]
    #[test]
    fn standard_game_roots_select_distribution_inventory_without_profile_mirror() {
        let composition =
            EngineCompositionSpec::new("test.standard-game", STANDARD_EXTERNAL_REQUIREMENTS)
                .with_runtime_unit_requirements(
                    newengine_runtime_units::STANDARD_GAME_RUNTIME_UNIT_REQUIREMENTS,
                );
        let catalog = distribution_runtime_unit_catalog().expect("distribution catalog");
        let descriptors = catalog.descriptors();
        let selected = select_runtime_unit_keys(composition, &descriptors, &[])
            .expect("standard game runtime-unit selection");
        for requirement in newengine_runtime_units::STANDARD_GAME_RUNTIME_UNIT_REQUIREMENTS {
            assert!(
                selected.iter().any(|key| {
                    catalog.registration(key).is_some_and(|entry| {
                        entry
                            .descriptor
                            .provides
                            .iter()
                            .any(|provided| provided == requirement.capability)
                    })
                }),
                "standard game root was not selected from distribution inventory: {}",
                requirement.capability
            );
        }
    }

    #[cfg(feature = "full-runtime")]
    #[test]
    fn distribution_catalog_contains_static_domain_units() {
        let catalog = distribution_runtime_unit_catalog().expect("distribution catalog");
        let ids = catalog
            .descriptors()
            .into_iter()
            .map(|descriptor| descriptor.id)
            .collect::<BTreeSet<_>>();
        for required in [
            "engine.runtime.scene",
            "engine.runtime.world",
            "engine.runtime.entity",
            "engine.runtime.time",
            "engine.runtime.schema",
            "engine.runtime.materials",
        ] {
            assert!(
                ids.contains(required),
                "missing distribution runtime unit {required}"
            );
        }
    }
    #[test]
    fn runtime_overlay_many_selects_all_compatible_units() {
        const FEATURE_A: EngineRuntimeUnitSpec = EngineRuntimeUnitSpec::new(
            "game.render.feature.a",
            1,
            EngineRuntimeUnitKind::ProductExtension,
            &["render.feature"],
            &[],
            &["engine.runtime-unit"],
        );
        const FEATURE_B: EngineRuntimeUnitSpec = EngineRuntimeUnitSpec::new(
            "game.render.feature.b",
            1,
            EngineRuntimeUnitKind::ProductExtension,
            &["render.feature"],
            &[],
            &["engine.runtime-unit"],
        );
        let descriptors = [FEATURE_A, FEATURE_B]
            .into_iter()
            .map(RuntimeUnitDescriptor::from_static)
            .collect::<Vec<_>>();
        let overlay = [RuntimeUnitRequirementDescriptor::required("render.feature")
            .with_cardinality(newengine_service_api::Cardinality::Many)];
        let selected = select_runtime_unit_keys(
            EngineCompositionSpec::new("test.game-module-overlay", &[]),
            &descriptors,
            &overlay,
        )
        .expect("many runtime-unit overlay must resolve");
        assert_eq!(selected.len(), 2);
        assert!(selected.contains(&"game.render.feature.a@1".to_owned()));
        assert!(selected.contains(&"game.render.feature.b@1".to_owned()));
    }
}
