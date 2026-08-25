use std::collections::{BTreeMap, BTreeSet};

use crate::{CapabilityRequirementLevel, EngineCapabilityRequirementSpec};

const PREFERRED_TAG_BONUS: i64 = 100;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompositionCandidate {
    pub gateway_id: String,
    pub capability_id: Option<String>,
    pub capability_version: Option<u32>,
    pub candidate_id: String,
    pub provider_owner_id: String,
    pub backend_priority: i32,
    pub origin_bias: i64,
    pub preference_bonus: i64,
    pub contract_id: Option<String>,
    pub contract_version: Option<u32>,
    pub tags: Vec<String>,
}

impl CompositionCandidate {
    #[inline]
    pub fn new(
        gateway_id: impl Into<String>,
        candidate_id: impl Into<String>,
        provider_owner_id: impl Into<String>,
        backend_priority: i32,
        origin_bias: i64,
        preference_bonus: i64,
    ) -> Self {
        Self {
            gateway_id: gateway_id.into(),
            capability_id: None,
            capability_version: None,
            candidate_id: candidate_id.into(),
            provider_owner_id: provider_owner_id.into(),
            backend_priority,
            origin_bias,
            preference_bonus,
            contract_id: None,
            contract_version: None,
            tags: Vec::new(),
        }
    }

    #[inline]
    pub fn with_contract_id(mut self, contract_id: impl Into<String>) -> Self {
        self.contract_id = Some(contract_id.into());
        self
    }

    #[inline]
    pub fn with_contract(mut self, contract_id: impl Into<String>, version: u32) -> Self {
        self.contract_id = Some(contract_id.into());
        self.contract_version = Some(version);
        self
    }

    #[inline]
    pub fn with_capability(mut self, capability_id: impl Into<String>) -> Self {
        self.capability_id = Some(capability_id.into());
        self
    }

    #[inline]
    pub fn with_capability_version(mut self, version: u32) -> Self {
        self.capability_version = Some(version);
        self
    }

    pub fn with_tags<I, S>(mut self, tags: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.tags = tags.into_iter().map(Into::into).collect();
        self.tags.sort();
        self.tags.dedup();
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompositionRequirement {
    pub capability_id: String,
    pub gateway_id: String,
    pub service_kind: String,
    pub level: CapabilityRequirementLevel,
    pub min_capability_version: u32,
    pub max_capability_version: Option<u32>,
    pub contract_id: Option<String>,
    pub min_contract_version: u32,
    pub max_contract_version: Option<u32>,
    pub required_tags: Vec<String>,
    pub preferred_tags: Vec<String>,
    pub conflict_tags: Vec<String>,
    pub fallback_provider_ids: Vec<String>,
    pub min_cardinality: u16,
    pub max_cardinality: u16,
    pub declared_by: String,
}

impl CompositionRequirement {
    pub fn from_spec(
        spec: &EngineCapabilityRequirementSpec,
        declared_by: impl Into<String>,
    ) -> Self {
        let min_cardinality = spec.cardinality.min(spec.strength);
        Self {
            capability_id: spec.capability.as_str().to_owned(),
            gateway_id: spec.capability.gateway_id().to_owned(),
            service_kind: spec.capability.service_kind().to_owned(),
            level: spec.strength,
            min_capability_version: 0,
            max_capability_version: None,
            contract_id: spec
                .contract
                .map(|contract| contract.contract_id.to_owned()),
            min_contract_version: spec
                .contract
                .map(|contract| contract.min_version)
                .unwrap_or(0),
            max_contract_version: spec.contract.and_then(|contract| contract.max_version),
            required_tags: spec
                .required_tags
                .iter()
                .map(|tag| tag.as_str().to_owned())
                .collect(),
            preferred_tags: spec
                .preferred_tags
                .iter()
                .map(|tag| tag.as_str().to_owned())
                .collect(),
            conflict_tags: spec
                .forbidden_tags
                .iter()
                .map(|tag| tag.as_str().to_owned())
                .collect(),
            fallback_provider_ids: spec
                .fallback
                .providers()
                .iter()
                .map(|provider| (*provider).to_owned())
                .collect(),
            min_cardinality,
            max_cardinality: spec.cardinality.max().max(min_cardinality),
            declared_by: declared_by.into(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CapabilityMatrix {
    requirements: Vec<CompositionRequirement>,
}

impl CapabilityMatrix {
    pub fn from_specs(
        declared_by: impl Into<String>,
        specs: &[EngineCapabilityRequirementSpec],
    ) -> Self {
        let declared_by = declared_by.into();
        Self::new(
            specs
                .iter()
                .map(|spec| CompositionRequirement::from_spec(spec, declared_by.clone()))
                .collect(),
        )
    }

    pub fn new(requirements: Vec<CompositionRequirement>) -> Self {
        let mut by_gateway = BTreeMap::<String, CompositionRequirement>::new();
        for requirement in requirements {
            match by_gateway.get_mut(&requirement.gateway_id) {
                None => {
                    by_gateway.insert(requirement.gateway_id.clone(), requirement);
                }
                Some(existing) => merge_requirement(existing, requirement),
            }
        }
        Self {
            requirements: by_gateway.into_values().collect(),
        }
    }

    #[inline]
    pub fn requirement(&self, gateway_id: &str) -> Option<&CompositionRequirement> {
        self.requirements
            .binary_search_by(|requirement| requirement.gateway_id.as_str().cmp(gateway_id))
            .ok()
            .map(|index| &self.requirements[index])
    }

    #[inline]
    pub fn requirements(&self) -> &[CompositionRequirement] {
        &self.requirements
    }
}

fn merge_requirement(existing: &mut CompositionRequirement, incoming: CompositionRequirement) {
    existing.level = existing.level.max(incoming.level);
    existing.min_cardinality = existing.min_cardinality.max(incoming.min_cardinality);
    existing.max_cardinality = existing
        .max_cardinality
        .min(incoming.max_cardinality)
        .max(existing.min_cardinality);

    existing.min_capability_version = existing
        .min_capability_version
        .max(incoming.min_capability_version);
    existing.max_capability_version = match (
        existing.max_capability_version,
        incoming.max_capability_version,
    ) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (Some(a), None) => Some(a),
        (None, Some(b)) => Some(b),
        (None, None) => None,
    };

    match (&existing.contract_id, &incoming.contract_id) {
        (None, Some(_)) => {
            existing.contract_id = incoming.contract_id.clone();
            existing.min_contract_version = incoming.min_contract_version;
            existing.max_contract_version = incoming.max_contract_version;
        }
        (Some(a), Some(b)) if a == b => {
            existing.min_contract_version = existing
                .min_contract_version
                .max(incoming.min_contract_version);
            existing.max_contract_version =
                match (existing.max_contract_version, incoming.max_contract_version) {
                    (Some(a), Some(b)) => Some(a.min(b)),
                    (Some(a), None) => Some(a),
                    (None, Some(b)) => Some(b),
                    (None, None) => None,
                };
        }
        (Some(a), Some(b)) if a != b => {
            existing.contract_id = Some(format!("__conflict__:{a}|{b}"));
            existing.min_contract_version = u32::MAX;
            existing.max_contract_version = Some(0);
        }
        _ => {}
    }

    merge_unique(&mut existing.required_tags, &incoming.required_tags);
    merge_unique(&mut existing.preferred_tags, &incoming.preferred_tags);
    merge_unique(&mut existing.conflict_tags, &incoming.conflict_tags);
    merge_unique(
        &mut existing.fallback_provider_ids,
        &incoming.fallback_provider_ids,
    );
    if existing.declared_by != incoming.declared_by {
        existing.declared_by = format!("{}+{}", existing.declared_by, incoming.declared_by);
    }
}

fn merge_unique(target: &mut Vec<String>, source: &[String]) {
    target.extend(source.iter().cloned());
    target.sort();
    target.dedup();
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompositionSelection {
    pub gateway_id: String,
    pub candidate_id: String,
    pub provider_owner_id: String,
    pub backend_priority: i32,
    pub origin_bias: i64,
    pub score: i64,
    pub fallback: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GatewayCompositionPlan {
    gateway_id: String,
    selected: Vec<CompositionSelection>,
    shadowed: Vec<CompositionSelection>,
}

impl GatewayCompositionPlan {
    #[inline]
    pub fn active(&self) -> Option<&CompositionSelection> {
        self.selected.first()
    }

    #[inline]
    pub fn selected(&self) -> &[CompositionSelection] {
        &self.selected
    }

    #[inline]
    pub fn shadowed(&self) -> &[CompositionSelection] {
        &self.shadowed
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnsatisfiedCapabilityRequirement {
    pub gateway_id: String,
    pub level: CapabilityRequirementLevel,
    pub required_min: u16,
    pub resolved: u16,
    pub declared_by: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CompositionPlan {
    gateways: Vec<GatewayCompositionPlan>,
    unsatisfied: Vec<UnsatisfiedCapabilityRequirement>,
}

impl CompositionPlan {
    #[inline]
    pub fn selected(&self, gateway_id: &str) -> Option<&CompositionSelection> {
        self.gateway_plan(gateway_id)
            .and_then(GatewayCompositionPlan::active)
    }

    #[inline]
    pub fn selected_all(&self, gateway_id: &str) -> &[CompositionSelection] {
        self.gateway_plan(gateway_id)
            .map(GatewayCompositionPlan::selected)
            .unwrap_or(&[])
    }

    #[inline]
    pub fn gateway_plan(&self, gateway_id: &str) -> Option<&GatewayCompositionPlan> {
        self.gateways
            .binary_search_by(|entry| entry.gateway_id.as_str().cmp(gateway_id))
            .ok()
            .map(|index| &self.gateways[index])
    }

    pub fn gateway_ids(&self) -> Vec<String> {
        self.gateways
            .iter()
            .map(|entry| entry.gateway_id.clone())
            .collect()
    }

    #[inline]
    pub fn unsatisfied(&self) -> &[UnsatisfiedCapabilityRequirement] {
        &self.unsatisfied
    }

    pub fn validate_required(&self) -> Result<(), String> {
        let missing = self
            .unsatisfied
            .iter()
            .filter(|missing| missing.level.is_required())
            .map(|missing| {
                format!(
                    "{}(resolved={}/{} declared_by={})",
                    missing.gateway_id, missing.resolved, missing.required_min, missing.declared_by
                )
            })
            .collect::<Vec<_>>();
        if missing.is_empty() {
            Ok(())
        } else {
            Err(format!(
                "required capability requirement(s) are unsatisfied: {}",
                missing.join(", ")
            ))
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct CompositionSolverInput {
    pub candidates: Vec<CompositionCandidate>,
    pub capability_matrix: CapabilityMatrix,
}

pub struct CompositionSolver;

impl CompositionSolver {
    #[inline]
    pub const fn score(origin_bias: i64, backend_priority: i32, preference_bonus: i64) -> i64 {
        origin_bias + backend_priority as i64 + preference_bonus
    }

    pub fn resolve<I>(candidates: I) -> CompositionPlan
    where
        I: IntoIterator<Item = CompositionCandidate>,
    {
        Self::resolve_input(CompositionSolverInput {
            candidates: candidates.into_iter().collect(),
            capability_matrix: CapabilityMatrix::default(),
        })
    }

    pub fn resolve_input(input: CompositionSolverInput) -> CompositionPlan {
        let mut by_gateway: BTreeMap<String, Vec<CompositionCandidate>> = BTreeMap::new();
        for candidate in input.candidates {
            if candidate.gateway_id.trim().is_empty() || candidate.candidate_id.trim().is_empty() {
                continue;
            }
            by_gateway
                .entry(candidate.gateway_id.clone())
                .or_default()
                .push(candidate);
        }
        for requirement in input.capability_matrix.requirements() {
            by_gateway
                .entry(requirement.gateway_id.clone())
                .or_default();
        }

        let mut gateways = Vec::with_capacity(by_gateway.len());
        let mut unsatisfied = Vec::new();
        for (gateway_id, candidates) in by_gateway {
            let requirement = input.capability_matrix.requirement(&gateway_id);
            let mut ranked = candidates
                .into_iter()
                .filter(|candidate| requirement.is_none_or(|req| candidate_matches(req, candidate)))
                .collect::<Vec<_>>();

            if let Some(req) = requirement {
                let has_non_fallback = ranked
                    .iter()
                    .any(|candidate| !is_fallback_candidate(req, candidate));
                if has_non_fallback {
                    ranked.retain(|candidate| !is_fallback_candidate(req, candidate));
                }
            }

            ranked.sort_by(|a, b| {
                candidate_score(requirement, b)
                    .cmp(&candidate_score(requirement, a))
                    .then_with(|| b.backend_priority.cmp(&a.backend_priority))
                    .then_with(|| b.origin_bias.cmp(&a.origin_bias))
                    .then_with(|| a.candidate_id.cmp(&b.candidate_id))
                    .then_with(|| a.provider_owner_id.cmp(&b.provider_owner_id))
            });

            let max = requirement
                .map(|req| usize::from(req.max_cardinality.max(1)))
                .unwrap_or(1);
            let selected_count = ranked.len().min(max);
            let selected = ranked
                .iter()
                .take(selected_count)
                .map(|candidate| selection_from_candidate(requirement, candidate))
                .collect::<Vec<_>>();
            let shadowed = ranked
                .iter()
                .skip(selected_count)
                .map(|candidate| selection_from_candidate(requirement, candidate))
                .collect::<Vec<_>>();

            if let Some(req) = requirement {
                if selected.len() < usize::from(req.min_cardinality) {
                    unsatisfied.push(UnsatisfiedCapabilityRequirement {
                        gateway_id: gateway_id.clone(),
                        level: req.level,
                        required_min: req.min_cardinality,
                        resolved: selected.len().min(usize::from(u16::MAX)) as u16,
                        declared_by: req.declared_by.clone(),
                    });
                }
            }

            gateways.push(GatewayCompositionPlan {
                gateway_id,
                selected,
                shadowed,
            });
        }

        CompositionPlan {
            gateways,
            unsatisfied,
        }
    }
}

fn candidate_score(
    requirement: Option<&CompositionRequirement>,
    candidate: &CompositionCandidate,
) -> i64 {
    let preferred_tag_bonus = requirement
        .map(|requirement| {
            requirement
                .preferred_tags
                .iter()
                .filter(|tag| {
                    candidate
                        .tags
                        .iter()
                        .any(|candidate_tag| candidate_tag == *tag)
                })
                .count() as i64
                * PREFERRED_TAG_BONUS
        })
        .unwrap_or(0);
    CompositionSolver::score(
        candidate.origin_bias,
        candidate.backend_priority,
        candidate.preference_bonus + preferred_tag_bonus,
    )
}

fn candidate_matches(
    requirement: &CompositionRequirement,
    candidate: &CompositionCandidate,
) -> bool {
    if let Some(candidate_capability) = candidate.capability_id.as_deref() {
        if candidate_capability != requirement.capability_id {
            return false;
        }
    }

    if requirement.min_capability_version > 0 || requirement.max_capability_version.is_some() {
        let Some(version) = candidate.capability_version else {
            return false;
        };
        if version < requirement.min_capability_version
            || requirement
                .max_capability_version
                .is_some_and(|max_version| version > max_version)
        {
            return false;
        }
    }

    if let Some(required_contract) = requirement.contract_id.as_deref() {
        if candidate.contract_id.as_deref() != Some(required_contract) {
            return false;
        }
        match candidate.contract_version {
            Some(version) => {
                if version < requirement.min_contract_version
                    || requirement
                        .max_contract_version
                        .is_some_and(|max_version| version > max_version)
                {
                    return false;
                }
            }
            None => {
                if requirement.min_contract_version > 0
                    || requirement.max_contract_version.is_some()
                {
                    return false;
                }
            }
        }
    }

    let tags = candidate
        .tags
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    if requirement
        .required_tags
        .iter()
        .any(|required| !tags.contains(required.as_str()))
    {
        return false;
    }
    if requirement
        .conflict_tags
        .iter()
        .any(|conflict| tags.contains(conflict.as_str()))
    {
        return false;
    }
    true
}

fn is_fallback_candidate(
    requirement: &CompositionRequirement,
    candidate: &CompositionCandidate,
) -> bool {
    requirement.fallback_provider_ids.iter().any(|fallback| {
        fallback == &candidate.candidate_id || fallback == &candidate.provider_owner_id
    })
}

fn selection_from_candidate(
    requirement: Option<&CompositionRequirement>,
    candidate: &CompositionCandidate,
) -> CompositionSelection {
    CompositionSelection {
        gateway_id: candidate.gateway_id.clone(),
        candidate_id: candidate.candidate_id.clone(),
        provider_owner_id: candidate.provider_owner_id.clone(),
        backend_priority: candidate.backend_priority,
        origin_bias: candidate.origin_bias,
        score: candidate_score(requirement, candidate),
        fallback: requirement.is_some_and(|req| is_fallback_candidate(req, candidate)),
    }
}

/// Parses ABI ids of the form `contract.name/v3` into (`contract.name`, 3).
pub fn parse_versioned_contract_id(value: &str) -> Option<(String, u32)> {
    let value = value.trim();
    let (contract, raw_version) = value.rsplit_once("/v")?;
    if contract.trim().is_empty() {
        return None;
    }
    let version = raw_version.parse::<u32>().ok()?;
    Some((contract.to_owned(), version))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        CapabilityId, CapabilityRequirement, Cardinality, FallbackPolicy, RequirementStrength,
        SystemTag,
    };

    const RENDER: CapabilityId = CapabilityId::new("render.backend", "engine.render", "render");
    const CODEC: CapabilityId = CapabilityId::new("codec.backend", "engine.codec", "codec");
    const SHADOW: SystemTag = SystemTag::new("feature.shadow");
    const SOFTWARE: SystemTag = SystemTag::new("backend.software");
    const TIMELINE: SystemTag = SystemTag::new("feature.timeline");

    fn candidate(id: &str, priority: i32) -> CompositionCandidate {
        CompositionCandidate::new("engine.render", id, id, priority, 20_000, 0)
            .with_capability("render.backend")
    }

    #[test]
    fn higher_score_wins() {
        let plan = CompositionSolver::resolve([candidate("low", 10), candidate("high", 20)]);
        assert_eq!(
            plan.selected("engine.render")
                .map(|selection| selection.candidate_id.as_str()),
            Some("high")
        );
    }

    #[test]
    fn version_tags_conflicts_and_fallback_are_resolver_input() {
        const SPEC: CapabilityRequirement = CapabilityRequirement::required(RENDER)
            .with_contract("newengine.render-provider", 2, Some(3))
            .with_required_tags(&[SHADOW])
            .with_forbidden_tags(&[SOFTWARE])
            .with_fallback(FallbackPolicy::Providers(&["render.null"]));
        let matrix = CapabilityMatrix::from_specs("test", &[SPEC]);
        let compatible = candidate("render.real", 10)
            .with_contract("newengine.render-provider", 2)
            .with_tags(["feature.shadow"]);
        let incompatible = candidate("render.bad", 1000)
            .with_contract("newengine.render-provider", 1)
            .with_tags(["feature.shadow"]);
        let forbidden = candidate("render.software", 2000)
            .with_contract("newengine.render-provider", 2)
            .with_tags(["feature.shadow", "backend.software"]);
        let fallback = candidate("render.null", -1000)
            .with_contract("newengine.render-provider", 2)
            .with_tags(["feature.shadow"]);
        let plan = CompositionSolver::resolve_input(CompositionSolverInput {
            candidates: vec![fallback, incompatible, forbidden, compatible],
            capability_matrix: matrix,
        });
        assert_eq!(
            plan.selected("engine.render")
                .map(|selection| selection.candidate_id.as_str()),
            Some("render.real")
        );
        assert!(plan.validate_required().is_ok());
    }

    #[test]
    fn required_many_requires_at_least_one_provider() {
        const SPEC: CapabilityRequirement =
            CapabilityRequirement::required(CODEC).with_cardinality(Cardinality::Many);
        let missing = CompositionSolver::resolve_input(CompositionSolverInput {
            candidates: Vec::new(),
            capability_matrix: CapabilityMatrix::from_specs("test", &[SPEC]),
        });
        assert!(missing.validate_required().is_err());

        let resolved = CompositionSolver::resolve_input(CompositionSolverInput {
            candidates: vec![
                CompositionCandidate::new("engine.codec", "codec.a", "codec.a", 0, 0, 0)
                    .with_capability("codec.backend"),
                CompositionCandidate::new("engine.codec", "codec.b", "codec.b", 0, 0, 0)
                    .with_capability("codec.backend"),
            ],
            capability_matrix: CapabilityMatrix::from_specs("test", &[SPEC]),
        });
        assert!(resolved.validate_required().is_ok());
        assert_eq!(resolved.selected_all("engine.codec").len(), 2);
    }

    #[test]
    fn duplicate_requirements_merge_to_strictest_matrix() {
        let required = CompositionRequirement::from_spec(
            &CapabilityRequirement::required(RENDER).with_required_tags(&[SHADOW]),
            "game",
        );
        let preferred = CompositionRequirement::from_spec(
            &CapabilityRequirement::preferred(RENDER).with_preferred_tags(&[TIMELINE]),
            "editor",
        );
        let matrix = CapabilityMatrix::new(vec![required, preferred]);
        assert_eq!(matrix.requirements().len(), 1);
        let requirement = matrix.requirement("engine.render").unwrap();
        assert_eq!(requirement.level, RequirementStrength::Required);
        assert!(requirement
            .required_tags
            .contains(&"feature.shadow".to_owned()));
        assert!(requirement
            .preferred_tags
            .contains(&"feature.timeline".to_owned()));
    }

    #[test]
    fn capability_version_and_typed_tag_constraints_drive_selection() {
        let mut requirement = CompositionRequirement::from_spec(
            &CapabilityRequirement::required(RENDER)
                .with_required_tags(&[SHADOW])
                .with_preferred_tags(&[TIMELINE])
                .with_forbidden_tags(&[SOFTWARE]),
            "typed-plugin",
        );
        requirement.min_capability_version = 2;
        requirement.max_capability_version = Some(3);
        let matrix = CapabilityMatrix::new(vec![requirement]);

        let preferred = candidate("render.preferred", 10)
            .with_capability_version(3)
            .with_tags(["feature.shadow", "feature.timeline"]);
        let plain = candidate("render.plain", 10)
            .with_capability_version(3)
            .with_tags(["feature.shadow"]);
        let too_new = candidate("render.too-new", 10_000)
            .with_capability_version(4)
            .with_tags(["feature.shadow", "feature.timeline"]);
        let forbidden = candidate("render.software", 20_000)
            .with_capability_version(3)
            .with_tags(["feature.shadow", "feature.timeline", "backend.software"]);

        let plan = CompositionSolver::resolve_input(CompositionSolverInput {
            candidates: vec![plain, forbidden, too_new, preferred],
            capability_matrix: matrix,
        });
        assert_eq!(
            plan.selected("engine.render")
                .map(|selection| selection.candidate_id.as_str()),
            Some("render.preferred")
        );
        assert!(plan.validate_required().is_ok());
    }

    #[test]
    fn parser_extracts_versioned_contract() {
        assert_eq!(
            parse_versioned_contract_id("newengine.render-provider/v12"),
            Some(("newengine.render-provider".to_owned(), 12))
        );
    }
}
