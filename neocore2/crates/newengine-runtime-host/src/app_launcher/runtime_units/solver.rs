use std::collections::{BTreeMap, BTreeSet};

use newengine_service_api::{
    CapabilityMatrix, CompositionCandidate, CompositionRequirement, CompositionSolver,
    CompositionSolverInput, EngineCompositionSpec, RuntimeUnitDescriptor,
    RuntimeUnitRequirementDescriptor, RuntimeUnitRequirementSpec,
};

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

pub(super) fn select_runtime_unit_keys(
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
