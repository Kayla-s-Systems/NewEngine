use std::collections::{BTreeMap, BTreeSet};

use crate::{CapabilityRequirementLevel, EngineCapabilityRequirementSpec};

const PREFERRED_TAG_BONUS: i64 = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompositionCandidateDisposition {
    Selected,
    Shadowed,
    Rejected,
}

impl CompositionCandidateDisposition {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Selected => "selected",
            Self::Shadowed => "shadowed",
            Self::Rejected => "rejected",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CompositionRejectionKind {
    FormatMismatch,
    ExternalPolicy,
    CompositionForbiddenTag,
    MissingCapability,
    MissingCapabilityVersion,
    CapabilityVersionBelowMinimum,
    CapabilityVersionAboveMaximum,
    ContractMismatch,
    MissingContractVersion,
    ContractVersionBelowMinimum,
    ContractVersionAboveMaximum,
    MissingRequiredTag,
    ForbiddenTag,
    FallbackSuppressed,
}

impl CompositionRejectionKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::FormatMismatch => "format_mismatch",
            Self::ExternalPolicy => "external_policy",
            Self::CompositionForbiddenTag => "composition_forbidden_tag",
            Self::MissingCapability => "missing_capability",
            Self::MissingCapabilityVersion => "missing_capability_version",
            Self::CapabilityVersionBelowMinimum => "capability_version_below_minimum",
            Self::CapabilityVersionAboveMaximum => "capability_version_above_maximum",
            Self::ContractMismatch => "contract_mismatch",
            Self::MissingContractVersion => "missing_contract_version",
            Self::ContractVersionBelowMinimum => "contract_version_below_minimum",
            Self::ContractVersionAboveMaximum => "contract_version_above_maximum",
            Self::MissingRequiredTag => "missing_required_tag",
            Self::ForbiddenTag => "forbidden_tag",
            Self::FallbackSuppressed => "fallback_suppressed",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct CompositionRejectionReason {
    pub kind: CompositionRejectionKind,
    pub capability_id: Option<String>,
    pub expected: Option<String>,
    pub actual: Option<String>,
}

impl CompositionRejectionReason {
    pub fn new(kind: CompositionRejectionKind) -> Self {
        Self {
            kind,
            capability_id: None,
            expected: None,
            actual: None,
        }
    }

    pub fn for_capability(mut self, capability_id: impl Into<String>) -> Self {
        self.capability_id = Some(capability_id.into());
        self
    }

    pub fn with_expected(mut self, expected: impl Into<String>) -> Self {
        self.expected = Some(expected.into());
        self
    }

    pub fn with_actual(mut self, actual: impl Into<String>) -> Self {
        self.actual = Some(actual.into());
        self
    }

    pub const fn code(&self) -> &'static str {
        self.kind.as_str()
    }

    pub fn summary(&self) -> String {
        let mut out = self.kind.as_str().to_owned();
        if let Some(capability_id) = self.capability_id.as_deref() {
            out.push_str(" capability=");
            out.push_str(capability_id);
        }
        if let Some(expected) = self.expected.as_deref() {
            out.push_str(" expected=");
            out.push_str(expected);
        }
        if let Some(actual) = self.actual.as_deref() {
            out.push_str(" actual=");
            out.push_str(actual);
        }
        out
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompositionRequirementEvaluation {
    pub capability_id: String,
    pub accepted: bool,
    pub rejection_reasons: Vec<CompositionRejectionReason>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CompositionScoreBreakdown {
    pub origin_bias: i64,
    pub backend_priority: i32,
    pub base_preference_bonus: i64,
    pub preferred_tag_matches: Vec<String>,
    pub preferred_tag_bonus: i64,
    pub total: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompositionCandidateExplanation {
    pub gateway_id: String,
    pub candidate_id: String,
    pub provider_owner_id: String,
    pub disposition: CompositionCandidateDisposition,
    /// One-based rank among compatible candidates after shared scoring/tie-breakers.
    /// Rejected candidates have no rank.
    pub rank: Option<usize>,
    /// Selected candidates that outrank this compatible shadowed candidate.
    pub outranked_by: Vec<String>,
    pub fallback: bool,
    pub score: CompositionScoreBreakdown,
    pub requirement_evaluations: Vec<CompositionRequirementEvaluation>,
    pub rejection_reasons: Vec<CompositionRejectionReason>,
}

impl CompositionCandidateExplanation {
    pub fn summary(&self) -> String {
        let preferred = if self.score.preferred_tag_matches.is_empty() {
            "none".to_owned()
        } else {
            self.score.preferred_tag_matches.join(",")
        };
        let rank = self
            .rank
            .map(|rank| rank.to_string())
            .unwrap_or_else(|| "none".to_owned());
        let outranked_by = if self.outranked_by.is_empty() {
            "none".to_owned()
        } else {
            self.outranked_by.join(",")
        };
        if self.rejection_reasons.is_empty() {
            format!(
                "{} rank={} outranked_by={} score={} origin_bias={} backend_priority={} preference_bonus={} preferred_tags={}",
                self.disposition.as_str(),
                rank,
                outranked_by,
                self.score.total,
                self.score.origin_bias,
                self.score.backend_priority,
                self.score.base_preference_bonus + self.score.preferred_tag_bonus,
                preferred
            )
        } else {
            format!(
                "{} reasons=[{}] rank={} outranked_by={} score={} origin_bias={} backend_priority={} preference_bonus={} preferred_tags={}",
                self.disposition.as_str(),
                self.rejection_reasons
                    .iter()
                    .map(CompositionRejectionReason::summary)
                    .collect::<Vec<_>>()
                    .join("; "),
                rank,
                outranked_by,
                self.score.total,
                self.score.origin_bias,
                self.score.backend_priority,
                self.score.base_preference_bonus + self.score.preferred_tag_bonus,
                preferred
            )
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GatewayCompositionExplanation {
    pub gateway_id: String,
    pub requirements: Vec<CompositionRequirement>,
    pub candidates: Vec<CompositionCandidateExplanation>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompositionContractResolutionSubject {
    Candidate,
    Requirement,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompositionContractResolution {
    pub subject: CompositionContractResolutionSubject,
    pub gateway_id: String,
    pub candidate_id: Option<String>,
    pub capability_id: String,
    pub reference: String,
    pub canonical_id: String,
    pub min_version: u32,
    pub max_version: Option<u32>,
    pub authority: String,
    pub owner: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CompositionExplanationGraph {
    gateways: Vec<GatewayCompositionExplanation>,
    contract_resolutions: Vec<CompositionContractResolution>,
}

impl CompositionExplanationGraph {
    pub fn gateway(&self, gateway_id: &str) -> Option<&GatewayCompositionExplanation> {
        self.gateways
            .binary_search_by(|entry| entry.gateway_id.as_str().cmp(gateway_id))
            .ok()
            .map(|index| &self.gateways[index])
    }

    pub fn gateways(&self) -> &[GatewayCompositionExplanation] {
        &self.gateways
    }

    pub fn contract_resolutions(&self) -> &[CompositionContractResolution] {
        &self.contract_resolutions
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompositionCapabilityMetadata {
    pub id: String,
    pub version: Option<u32>,
    pub contract_id: Option<String>,
    pub contract_version: Option<u32>,
    pub tags: Vec<String>,
}

impl CompositionCapabilityMetadata {
    #[inline]
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            version: None,
            contract_id: None,
            contract_version: None,
            tags: Vec::new(),
        }
    }

    #[inline]
    pub fn with_version(mut self, version: u32) -> Self {
        self.version = Some(version);
        self
    }

    #[inline]
    pub fn with_contract(mut self, contract_id: impl Into<String>, version: Option<u32>) -> Self {
        self.contract_id = Some(contract_id.into());
        self.contract_version = version;
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
pub struct CompositionCandidate {
    pub gateway_id: String,
    pub capability_id: Option<String>,
    /// Complete capability set advertised by this provider candidate. `capability_id`
    /// remains as the V1/single-capability compatibility field.
    pub capability_ids: Vec<String>,
    /// Lossless V2 metadata for each advertised capability. Legacy callers may
    /// continue using capability_id/capability_ids + the candidate-wide fields.
    pub capability_metadata: Vec<CompositionCapabilityMetadata>,
    pub capability_version: Option<u32>,
    pub candidate_id: String,
    pub provider_owner_id: String,
    pub backend_priority: i32,
    pub origin_bias: i64,
    pub preference_bonus: i64,
    pub contract_id: Option<String>,
    pub contract_version: Option<u32>,
    pub tags: Vec<String>,
    /// Domain adapters may record non-solver eligibility failures (for example
    /// an Editor format/extension mismatch). The shared solver owns the final
    /// rejection graph and never requires consumers to reconstruct these reasons.
    pub preflight_rejections: Vec<CompositionRejectionReason>,
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
            capability_ids: Vec::new(),
            capability_metadata: Vec::new(),
            capability_version: None,
            candidate_id: candidate_id.into(),
            provider_owner_id: provider_owner_id.into(),
            backend_priority,
            origin_bias,
            preference_bonus,
            contract_id: None,
            contract_version: None,
            tags: Vec::new(),
            preflight_rejections: Vec::new(),
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
        let capability_id = capability_id.into();
        self.capability_id = Some(capability_id.clone());
        self.capability_ids.push(capability_id);
        self.capability_ids.sort();
        self.capability_ids.dedup();
        self
    }

    /// Advertises every capability implemented by one provider candidate. This is
    /// required when one routed provider must satisfy multiple typed requirements
    /// on the same gateway (for example an editor format provider that must read,
    /// inspect and preview the same asset type).
    pub fn with_capabilities<I, S>(mut self, capability_ids: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.capability_ids
            .extend(capability_ids.into_iter().map(Into::into));
        self.capability_ids.sort();
        self.capability_ids.dedup();
        if self.capability_id.is_none() {
            self.capability_id = self.capability_ids.first().cloned();
        }
        self
    }

    #[inline]
    pub fn with_capability_version(mut self, version: u32) -> Self {
        self.capability_version = Some(version);
        self
    }

    pub fn with_capability_metadata<I>(mut self, capabilities: I) -> Self
    where
        I: IntoIterator<Item = CompositionCapabilityMetadata>,
    {
        self.capability_metadata.extend(capabilities);
        self.capability_metadata.sort_by(|a, b| a.id.cmp(&b.id));
        self.capability_metadata.dedup_by(|a, b| a.id == b.id);
        self.capability_ids.extend(
            self.capability_metadata
                .iter()
                .map(|capability| capability.id.clone()),
        );
        self.capability_ids.sort();
        self.capability_ids.dedup();
        if self.capability_id.is_none() {
            self.capability_id = self.capability_ids.first().cloned();
        }
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

    pub fn with_preflight_rejection(mut self, reason: CompositionRejectionReason) -> Self {
        self.preflight_rejections.push(reason);
        self.preflight_rejections.sort();
        self.preflight_rejections.dedup();
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
    /// Backward-compatible gateway aggregate used by existing runtime callers for
    /// cardinality/fallback policy.
    requirements: Vec<CompositionRequirement>,
    /// Lossless typed requirements, merged only when both gateway and capability
    /// identity match. The solver uses this set for compatibility filtering.
    capability_requirements: Vec<CompositionRequirement>,
    preferred_tags: Vec<String>,
    conflict_tags: Vec<String>,
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

    pub fn from_composition(composition: &crate::EngineCompositionSpec) -> Self {
        Self::from_specs(composition.id, composition.requirements)
            .with_preferred_tags(composition.preferred_tags.iter().map(|tag| tag.as_str()))
            .with_forbidden_tags(composition.forbidden_tags.iter().map(|tag| tag.as_str()))
    }

    pub fn new(requirements: Vec<CompositionRequirement>) -> Self {
        let mut by_capability = BTreeMap::<(String, String), CompositionRequirement>::new();
        for requirement in requirements {
            let key = (
                requirement.gateway_id.clone(),
                requirement.capability_id.clone(),
            );
            match by_capability.get_mut(&key) {
                None => {
                    by_capability.insert(key, requirement);
                }
                Some(existing) => merge_requirement(existing, requirement),
            }
        }
        let capability_requirements = by_capability.into_values().collect::<Vec<_>>();

        // Preserve the historical one-entry-per-gateway view for callers that own
        // gateway slots. The authoritative solver filtering below uses the lossless
        // capability_requirements collection instead.
        let mut by_gateway = BTreeMap::<String, CompositionRequirement>::new();
        for requirement in capability_requirements.iter().cloned() {
            match by_gateway.get_mut(&requirement.gateway_id) {
                None => {
                    by_gateway.insert(requirement.gateway_id.clone(), requirement);
                }
                Some(existing) => merge_requirement(existing, requirement),
            }
        }
        Self {
            requirements: by_gateway.into_values().collect(),
            capability_requirements,
            preferred_tags: Vec::new(),
            conflict_tags: Vec::new(),
        }
    }

    pub fn with_preferred_tags<I, S>(mut self, tags: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.preferred_tags.extend(tags.into_iter().map(Into::into));
        self.preferred_tags.sort();
        self.preferred_tags.dedup();
        self
    }

    pub fn with_forbidden_tags<I, S>(mut self, tags: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.conflict_tags.extend(tags.into_iter().map(Into::into));
        self.conflict_tags.sort();
        self.conflict_tags.dedup();
        self
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

    /// Lossless typed requirements before gateway aggregation. Multiple capability
    /// identities may intentionally target the same gateway.
    #[inline]
    pub fn capability_requirements(&self) -> &[CompositionRequirement] {
        &self.capability_requirements
    }

    pub fn requirements_for_gateway(&self, gateway_id: &str) -> Vec<&CompositionRequirement> {
        self.capability_requirements
            .iter()
            .filter(|requirement| requirement.gateway_id == gateway_id)
            .collect()
    }

    #[inline]
    pub fn preferred_tags(&self) -> &[String] {
        &self.preferred_tags
    }

    #[inline]
    pub fn conflict_tags(&self) -> &[String] {
        &self.conflict_tags
    }

    #[inline]
    pub fn allows_system_tags(&self, tags: &[String]) -> bool {
        !self
            .conflict_tags
            .iter()
            .any(|conflict| tags.iter().any(|tag| tag == conflict))
    }

    #[inline]
    pub fn preferred_system_tag_matches(&self, tags: &[String]) -> usize {
        self.preferred_tags
            .iter()
            .filter(|preferred| tags.iter().any(|tag| tag == *preferred))
            .count()
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
    explanation: CompositionExplanationGraph,
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

    pub fn explanation(&self) -> &CompositionExplanationGraph {
        &self.explanation
    }

    pub fn with_contract_resolutions<I>(mut self, resolutions: I) -> Self
    where
        I: IntoIterator<Item = CompositionContractResolution>,
    {
        self.explanation.contract_resolutions.extend(resolutions);
        self.explanation.contract_resolutions.sort_by(|a, b| {
            a.gateway_id
                .cmp(&b.gateway_id)
                .then_with(|| a.candidate_id.cmp(&b.candidate_id))
                .then_with(|| a.capability_id.cmp(&b.capability_id))
                .then_with(|| a.reference.cmp(&b.reference))
        });
        self.explanation.contract_resolutions.dedup();
        self
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
        let CompositionSolverInput {
            candidates,
            capability_matrix,
        } = input;
        let mut by_gateway: BTreeMap<String, Vec<CompositionCandidate>> = BTreeMap::new();
        for candidate in candidates {
            if candidate.gateway_id.trim().is_empty() || candidate.candidate_id.trim().is_empty() {
                continue;
            }
            by_gateway
                .entry(candidate.gateway_id.clone())
                .or_default()
                .push(candidate);
        }
        for requirement in capability_matrix.capability_requirements() {
            by_gateway
                .entry(requirement.gateway_id.clone())
                .or_default();
        }

        let mut gateways = Vec::with_capacity(by_gateway.len());
        let mut explanations = Vec::with_capacity(by_gateway.len());
        let mut unsatisfied = Vec::new();
        for (gateway_id, candidates) in by_gateway {
            let requirement = capability_matrix.requirement(&gateway_id);
            let typed_requirements = capability_matrix.requirements_for_gateway(&gateway_id);
            let mut candidate_explanations = candidates
                .iter()
                .map(|candidate| {
                    explain_candidate(
                        &capability_matrix,
                        requirement,
                        &typed_requirements,
                        candidate,
                    )
                })
                .collect::<Vec<_>>();

            let mut ranked = candidates
                .into_iter()
                .filter(|candidate| {
                    candidate.preflight_rejections.is_empty()
                        && candidate_matches_composition(&capability_matrix, candidate)
                        && typed_requirements
                            .iter()
                            .all(|req| candidate_matches(req, candidate))
                })
                .collect::<Vec<_>>();

            if let Some(req) = requirement {
                let has_non_fallback = ranked
                    .iter()
                    .any(|candidate| !is_fallback_candidate(req, candidate));
                if has_non_fallback {
                    let suppressed = ranked
                        .iter()
                        .filter(|candidate| is_fallback_candidate(req, candidate))
                        .map(|candidate| candidate.candidate_id.clone())
                        .collect::<BTreeSet<_>>();
                    ranked.retain(|candidate| !suppressed.contains(&candidate.candidate_id));
                    for explanation in &mut candidate_explanations {
                        if suppressed.contains(&explanation.candidate_id) {
                            explanation.disposition = CompositionCandidateDisposition::Rejected;
                            push_reason_unique(
                                &mut explanation.rejection_reasons,
                                CompositionRejectionReason::new(
                                    CompositionRejectionKind::FallbackSuppressed,
                                )
                                .with_expected("non-fallback candidate available"),
                            );
                        }
                    }
                }
            }

            ranked.sort_by(|a, b| {
                candidate_score(&capability_matrix, requirement, b)
                    .cmp(&candidate_score(&capability_matrix, requirement, a))
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
                .map(|candidate| {
                    selection_from_candidate(&capability_matrix, requirement, candidate)
                })
                .collect::<Vec<_>>();
            let shadowed = ranked
                .iter()
                .skip(selected_count)
                .map(|candidate| {
                    selection_from_candidate(&capability_matrix, requirement, candidate)
                })
                .collect::<Vec<_>>();

            let selected_ids = selected
                .iter()
                .map(|entry| entry.candidate_id.clone())
                .collect::<Vec<_>>();
            let rank_by_id = ranked
                .iter()
                .enumerate()
                .map(|(index, candidate)| (candidate.candidate_id.as_str(), index + 1))
                .collect::<BTreeMap<_, _>>();
            let selected_id_set = selected_ids
                .iter()
                .map(String::as_str)
                .collect::<BTreeSet<_>>();
            let shadowed_ids = shadowed
                .iter()
                .map(|entry| entry.candidate_id.as_str())
                .collect::<BTreeSet<_>>();
            for explanation in &mut candidate_explanations {
                explanation.rank = rank_by_id.get(explanation.candidate_id.as_str()).copied();
                if selected_id_set.contains(explanation.candidate_id.as_str()) {
                    explanation.disposition = CompositionCandidateDisposition::Selected;
                } else if shadowed_ids.contains(explanation.candidate_id.as_str()) {
                    explanation.disposition = CompositionCandidateDisposition::Shadowed;
                    explanation.outranked_by = selected_ids.clone();
                }
            }
            candidate_explanations.sort_by(|a, b| {
                disposition_rank(a.disposition)
                    .cmp(&disposition_rank(b.disposition))
                    .then_with(|| b.score.total.cmp(&a.score.total))
                    .then_with(|| a.candidate_id.cmp(&b.candidate_id))
            });

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

            explanations.push(GatewayCompositionExplanation {
                gateway_id: gateway_id.clone(),
                requirements: typed_requirements.into_iter().cloned().collect(),
                candidates: candidate_explanations,
            });
            gateways.push(GatewayCompositionPlan {
                gateway_id,
                selected,
                shadowed,
            });
        }

        CompositionPlan {
            gateways,
            unsatisfied,
            explanation: CompositionExplanationGraph {
                gateways: explanations,
                contract_resolutions: Vec::new(),
            },
        }
    }
}

fn candidate_score(
    matrix: &CapabilityMatrix,
    requirement: Option<&CompositionRequirement>,
    candidate: &CompositionCandidate,
) -> i64 {
    candidate_score_breakdown(matrix, requirement, candidate).total
}

fn candidate_score_breakdown(
    matrix: &CapabilityMatrix,
    requirement: Option<&CompositionRequirement>,
    candidate: &CompositionCandidate,
) -> CompositionScoreBreakdown {
    let mut preferred_tags = matrix
        .preferred_tags()
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    if let Some(requirement) = requirement {
        preferred_tags.extend(requirement.preferred_tags.iter().map(String::as_str));
    }
    let candidate_tags = candidate_all_tags(candidate);
    let preferred_tag_matches = preferred_tags
        .iter()
        .filter(|tag| candidate_tags.contains(**tag))
        .map(|tag| (*tag).to_owned())
        .collect::<Vec<_>>();
    let preferred_tag_bonus = preferred_tag_matches.len() as i64 * PREFERRED_TAG_BONUS;
    let total = CompositionSolver::score(
        candidate.origin_bias,
        candidate.backend_priority,
        candidate.preference_bonus + preferred_tag_bonus,
    );
    CompositionScoreBreakdown {
        origin_bias: candidate.origin_bias,
        backend_priority: candidate.backend_priority,
        base_preference_bonus: candidate.preference_bonus,
        preferred_tag_matches,
        preferred_tag_bonus,
        total,
    }
}

fn candidate_all_tags(candidate: &CompositionCandidate) -> BTreeSet<&str> {
    candidate
        .tags
        .iter()
        .map(String::as_str)
        .chain(
            candidate
                .capability_metadata
                .iter()
                .flat_map(|capability| capability.tags.iter().map(String::as_str)),
        )
        .collect()
}

fn candidate_tags_for_requirement<'a>(
    candidate: &'a CompositionCandidate,
    capability: Option<&'a CompositionCapabilityMetadata>,
) -> BTreeSet<&'a str> {
    candidate
        .tags
        .iter()
        .map(String::as_str)
        .chain(
            capability
                .into_iter()
                .flat_map(|capability| capability.tags.iter().map(String::as_str)),
        )
        .collect()
}

fn disposition_rank(disposition: CompositionCandidateDisposition) -> u8 {
    match disposition {
        CompositionCandidateDisposition::Selected => 0,
        CompositionCandidateDisposition::Shadowed => 1,
        CompositionCandidateDisposition::Rejected => 2,
    }
}

fn push_reason_unique(
    reasons: &mut Vec<CompositionRejectionReason>,
    reason: CompositionRejectionReason,
) {
    if !reasons.contains(&reason) {
        reasons.push(reason);
    }
}

fn explain_candidate(
    matrix: &CapabilityMatrix,
    requirement: Option<&CompositionRequirement>,
    typed_requirements: &[&CompositionRequirement],
    candidate: &CompositionCandidate,
) -> CompositionCandidateExplanation {
    let mut rejection_reasons = candidate.preflight_rejections.clone();
    let tags = candidate_all_tags(candidate);
    for conflict in matrix.conflict_tags() {
        if tags.contains(conflict.as_str()) {
            push_reason_unique(
                &mut rejection_reasons,
                CompositionRejectionReason::new(CompositionRejectionKind::CompositionForbiddenTag)
                    .with_expected(format!("tag '{}' must be absent", conflict))
                    .with_actual(conflict.clone()),
            );
        }
    }

    let requirement_evaluations = typed_requirements
        .iter()
        .map(|req| {
            let reasons = candidate_requirement_rejections(req, candidate);
            for reason in &reasons {
                push_reason_unique(&mut rejection_reasons, reason.clone());
            }
            CompositionRequirementEvaluation {
                capability_id: req.capability_id.clone(),
                accepted: reasons.is_empty(),
                rejection_reasons: reasons,
            }
        })
        .collect::<Vec<_>>();

    CompositionCandidateExplanation {
        gateway_id: candidate.gateway_id.clone(),
        candidate_id: candidate.candidate_id.clone(),
        provider_owner_id: candidate.provider_owner_id.clone(),
        disposition: if rejection_reasons.is_empty() {
            CompositionCandidateDisposition::Shadowed
        } else {
            CompositionCandidateDisposition::Rejected
        },
        rank: None,
        outranked_by: Vec::new(),
        fallback: requirement.is_some_and(|req| is_fallback_candidate(req, candidate)),
        score: candidate_score_breakdown(matrix, requirement, candidate),
        requirement_evaluations,
        rejection_reasons,
    }
}

fn candidate_requirement_rejections(
    requirement: &CompositionRequirement,
    candidate: &CompositionCandidate,
) -> Vec<CompositionRejectionReason> {
    let mut reasons = Vec::new();
    let typed_capability = candidate
        .capability_metadata
        .iter()
        .find(|capability| capability.id == requirement.capability_id);

    let capability_present = if !candidate.capability_metadata.is_empty() {
        typed_capability.is_some()
    } else if candidate.capability_id.is_some() || !candidate.capability_ids.is_empty() {
        candidate
            .capability_id
            .as_deref()
            .is_some_and(|capability| capability == requirement.capability_id)
            || candidate
                .capability_ids
                .iter()
                .any(|capability| capability == &requirement.capability_id)
    } else {
        true
    };
    if !capability_present {
        reasons.push(
            CompositionRejectionReason::new(CompositionRejectionKind::MissingCapability)
                .for_capability(requirement.capability_id.clone())
                .with_expected(requirement.capability_id.clone()),
        );
        return reasons;
    }

    if requirement.min_capability_version > 0 || requirement.max_capability_version.is_some() {
        let version = typed_capability
            .and_then(|capability| capability.version)
            .or(candidate.capability_version);
        match version {
            None => reasons.push(
                CompositionRejectionReason::new(CompositionRejectionKind::MissingCapabilityVersion)
                    .for_capability(requirement.capability_id.clone())
                    .with_expected(format!(
                        "{}..{}",
                        requirement.min_capability_version,
                        requirement
                            .max_capability_version
                            .map(|value| value.to_string())
                            .unwrap_or_else(|| "*".to_owned())
                    )),
            ),
            Some(version) if version < requirement.min_capability_version => reasons.push(
                CompositionRejectionReason::new(
                    CompositionRejectionKind::CapabilityVersionBelowMinimum,
                )
                .for_capability(requirement.capability_id.clone())
                .with_expected(format!(">={}", requirement.min_capability_version))
                .with_actual(version.to_string()),
            ),
            Some(version)
                if requirement
                    .max_capability_version
                    .is_some_and(|max_version| version > max_version) =>
            {
                reasons.push(
                    CompositionRejectionReason::new(
                        CompositionRejectionKind::CapabilityVersionAboveMaximum,
                    )
                    .for_capability(requirement.capability_id.clone())
                    .with_expected(format!("<={}", requirement.max_capability_version.unwrap()))
                    .with_actual(version.to_string()),
                );
            }
            _ => {}
        }
    }

    if let Some(required_contract) = requirement.contract_id.as_deref() {
        let contract_id = typed_capability
            .and_then(|capability| capability.contract_id.as_deref())
            .or(candidate.contract_id.as_deref());
        if contract_id != Some(required_contract) {
            reasons.push(
                CompositionRejectionReason::new(CompositionRejectionKind::ContractMismatch)
                    .for_capability(requirement.capability_id.clone())
                    .with_expected(required_contract.to_owned())
                    .with_actual(contract_id.unwrap_or("<none>").to_owned()),
            );
        } else {
            let contract_version = typed_capability
                .and_then(|capability| capability.contract_version)
                .or(candidate.contract_version);
            match contract_version {
                Some(version) if version < requirement.min_contract_version => reasons.push(
                    CompositionRejectionReason::new(
                        CompositionRejectionKind::ContractVersionBelowMinimum,
                    )
                    .for_capability(requirement.capability_id.clone())
                    .with_expected(format!(">={}", requirement.min_contract_version))
                    .with_actual(version.to_string()),
                ),
                Some(version)
                    if requirement
                        .max_contract_version
                        .is_some_and(|max_version| version > max_version) =>
                {
                    reasons.push(
                        CompositionRejectionReason::new(
                            CompositionRejectionKind::ContractVersionAboveMaximum,
                        )
                        .for_capability(requirement.capability_id.clone())
                        .with_expected(format!("<={}", requirement.max_contract_version.unwrap()))
                        .with_actual(version.to_string()),
                    );
                }
                None if requirement.min_contract_version > 0
                    || requirement.max_contract_version.is_some() =>
                {
                    reasons.push(
                        CompositionRejectionReason::new(
                            CompositionRejectionKind::MissingContractVersion,
                        )
                        .for_capability(requirement.capability_id.clone())
                        .with_expected(format!(
                            "{}..{}",
                            requirement.min_contract_version,
                            requirement
                                .max_contract_version
                                .map(|value| value.to_string())
                                .unwrap_or_else(|| "*".to_owned())
                        )),
                    );
                }
                _ => {}
            }
        }
    }

    let tags = candidate_tags_for_requirement(candidate, typed_capability);
    for required in &requirement.required_tags {
        if !tags.contains(required.as_str()) {
            reasons.push(
                CompositionRejectionReason::new(CompositionRejectionKind::MissingRequiredTag)
                    .for_capability(requirement.capability_id.clone())
                    .with_expected(required.clone()),
            );
        }
    }
    for conflict in &requirement.conflict_tags {
        if tags.contains(conflict.as_str()) {
            reasons.push(
                CompositionRejectionReason::new(CompositionRejectionKind::ForbiddenTag)
                    .for_capability(requirement.capability_id.clone())
                    .with_expected(format!("tag '{}' must be absent", conflict))
                    .with_actual(conflict.clone()),
            );
        }
    }
    reasons.sort();
    reasons.dedup();
    reasons
}

fn candidate_matches_composition(
    matrix: &CapabilityMatrix,
    candidate: &CompositionCandidate,
) -> bool {
    let tags = candidate_all_tags(candidate);
    !matrix
        .conflict_tags()
        .iter()
        .any(|conflict| tags.contains(conflict.as_str()))
}

fn candidate_matches(
    requirement: &CompositionRequirement,
    candidate: &CompositionCandidate,
) -> bool {
    candidate_requirement_rejections(requirement, candidate).is_empty()
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
    matrix: &CapabilityMatrix,
    requirement: Option<&CompositionRequirement>,
    candidate: &CompositionCandidate,
) -> CompositionSelection {
    CompositionSelection {
        gateway_id: candidate.gateway_id.clone(),
        candidate_id: candidate.candidate_id.clone(),
        provider_owner_id: candidate.provider_owner_id.clone(),
        backend_priority: candidate.backend_priority,
        origin_bias: candidate.origin_bias,
        score: candidate_score(matrix, requirement, candidate),
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
    fn composition_wide_forbidden_tags_filter_candidates_without_gateway_requirement() {
        let headful = CompositionCandidate::new(
            "vendor.presentation",
            "vendor.alpha",
            "vendor.alpha",
            100,
            0,
            0,
        )
        .with_tags(["headful"]);
        let headless =
            CompositionCandidate::new("vendor.presentation", "vendor.beta", "vendor.beta", 1, 0, 0)
                .with_tags(["headless", "deterministic"]);

        let plan = CompositionSolver::resolve_input(CompositionSolverInput {
            candidates: vec![headful, headless],
            capability_matrix: CapabilityMatrix::default().with_forbidden_tags(["headful"]),
        });

        assert_eq!(
            plan.selected("vendor.presentation")
                .map(|selection| selection.candidate_id.as_str()),
            Some("vendor.beta")
        );
    }

    #[test]
    fn composition_wide_preferred_tags_rank_candidates_without_provider_name_knowledge() {
        let implementation_named_like_render = CompositionCandidate::new(
            "vendor.output",
            "engine.render.vulkan",
            "engine.render.vulkan",
            10,
            0,
            0,
        );
        let tagged_headless = CompositionCandidate::new(
            "vendor.output",
            "vendor.null-output",
            "vendor.null-output",
            10,
            0,
            0,
        )
        .with_tags(["headless"]);

        let plan = CompositionSolver::resolve_input(CompositionSolverInput {
            candidates: vec![implementation_named_like_render, tagged_headless],
            capability_matrix: CapabilityMatrix::default().with_preferred_tags(["headless"]),
        });

        assert_eq!(
            plan.selected("vendor.output")
                .map(|selection| selection.candidate_id.as_str()),
            Some("vendor.null-output")
        );
    }

    #[test]
    fn provider_name_does_not_imply_headful_policy() {
        let provider = CompositionCandidate::new(
            "vendor.output",
            "engine.render.vulkan",
            "engine.render.vulkan",
            10,
            0,
            0,
        );
        let plan = CompositionSolver::resolve_input(CompositionSolverInput {
            candidates: vec![provider],
            capability_matrix: CapabilityMatrix::default().with_forbidden_tags(["headful"]),
        });
        assert_eq!(
            plan.selected("vendor.output")
                .map(|selection| selection.candidate_id.as_str()),
            Some("engine.render.vulkan")
        );
    }

    #[test]
    fn multiple_capabilities_on_one_gateway_require_one_provider_to_satisfy_all() {
        const READ: CapabilityId =
            CapabilityId::new("asset.format.read", "editor.preview", "editor.format");
        const PREVIEW: CapabilityId =
            CapabilityId::new("asset.preview.texture", "editor.preview", "editor.format");
        let matrix = CapabilityMatrix::from_specs(
            "editor.format.texture",
            &[
                CapabilityRequirement::required(READ),
                CapabilityRequirement::required(PREVIEW),
            ],
        );
        assert_eq!(matrix.capability_requirements().len(), 2);

        let partial = CompositionCandidate::new(
            "editor.preview",
            "provider.partial",
            "provider.partial",
            10_000,
            0,
            0,
        )
        .with_capabilities(["asset.format.read"]);
        let complete = CompositionCandidate::new(
            "editor.preview",
            "provider.complete",
            "provider.complete",
            0,
            0,
            0,
        )
        .with_capabilities(["asset.format.read", "asset.preview.texture"]);

        let plan = CompositionSolver::resolve_input(CompositionSolverInput {
            candidates: vec![partial, complete],
            capability_matrix: matrix,
        });
        assert_eq!(
            plan.selected("editor.preview")
                .map(|selection| selection.candidate_id.as_str()),
            Some("provider.complete")
        );
        assert!(plan.validate_required().is_ok());
    }

    #[test]
    fn per_capability_v2_metadata_drives_version_contract_and_tags() {
        let mut read = CompositionRequirement::from_spec(
            &CapabilityRequirement::required(RENDER).with_required_tags(&[SHADOW]),
            "editor",
        );
        read.capability_id = "asset.format.read".to_owned();
        read.min_capability_version = 2;
        read.contract_id = Some("editor.asset.read".to_owned());
        read.min_contract_version = 3;

        let mut preview = read.clone();
        preview.capability_id = "asset.preview.texture".to_owned();
        preview.min_capability_version = 4;
        preview.contract_id = Some("editor.asset.preview".to_owned());
        preview.min_contract_version = 5;

        let compatible =
            CompositionCandidate::new("engine.render", "provider.v2", "provider.v2", 0, 0, 0)
                .with_capability_metadata([
                    CompositionCapabilityMetadata::new("asset.format.read")
                        .with_version(2)
                        .with_contract("editor.asset.read", Some(3))
                        .with_tags(["feature.shadow"]),
                    CompositionCapabilityMetadata::new("asset.preview.texture")
                        .with_version(4)
                        .with_contract("editor.asset.preview", Some(5))
                        .with_tags(["feature.shadow"]),
                ]);
        let wrong_preview_version =
            CompositionCandidate::new("engine.render", "provider.old", "provider.old", 1000, 0, 0)
                .with_capability_metadata([
                    CompositionCapabilityMetadata::new("asset.format.read")
                        .with_version(2)
                        .with_contract("editor.asset.read", Some(3))
                        .with_tags(["feature.shadow"]),
                    CompositionCapabilityMetadata::new("asset.preview.texture")
                        .with_version(3)
                        .with_contract("editor.asset.preview", Some(5))
                        .with_tags(["feature.shadow"]),
                ]);

        let plan = CompositionSolver::resolve_input(CompositionSolverInput {
            candidates: vec![wrong_preview_version, compatible],
            capability_matrix: CapabilityMatrix::new(vec![read, preview]),
        });
        assert_eq!(
            plan.selected("engine.render")
                .map(|selection| selection.candidate_id.as_str()),
            Some("provider.v2")
        );
    }

    #[test]
    fn explanation_graph_reports_rejections_shadowing_and_score_breakdown() {
        let mut requirement = CompositionRequirement::from_spec(
            &CapabilityRequirement::required(RENDER)
                .with_required_tags(&[SHADOW])
                .with_preferred_tags(&[TIMELINE]),
            "explain-test",
        );
        requirement.min_capability_version = 2;
        let matrix = CapabilityMatrix::new(vec![requirement]);

        let selected = candidate("render.selected", 20)
            .with_capability_version(2)
            .with_tags(["feature.shadow", "feature.timeline"]);
        let shadowed = candidate("render.shadowed", 10)
            .with_capability_version(2)
            .with_tags(["feature.shadow"]);
        let rejected = candidate("render.rejected", 10_000)
            .with_capability_version(1)
            .with_tags(["feature.timeline"]);

        let plan = CompositionSolver::resolve_input(CompositionSolverInput {
            candidates: vec![rejected, shadowed, selected],
            capability_matrix: matrix,
        });
        let explanation = plan.explanation().gateway("engine.render").unwrap();
        let selected = explanation
            .candidates
            .iter()
            .find(|candidate| candidate.candidate_id == "render.selected")
            .unwrap();
        assert_eq!(
            selected.disposition,
            CompositionCandidateDisposition::Selected
        );
        assert_eq!(selected.score.backend_priority, 20);
        assert_eq!(selected.score.preferred_tag_bonus, PREFERRED_TAG_BONUS);
        assert_eq!(selected.score.total, 20_000 + 20 + PREFERRED_TAG_BONUS);

        let shadowed = explanation
            .candidates
            .iter()
            .find(|candidate| candidate.candidate_id == "render.shadowed")
            .unwrap();
        assert_eq!(
            shadowed.disposition,
            CompositionCandidateDisposition::Shadowed
        );
        assert_eq!(shadowed.rank, Some(2));
        assert_eq!(shadowed.outranked_by, vec!["render.selected".to_owned()]);
        assert!(shadowed.rejection_reasons.is_empty());

        let rejected = explanation
            .candidates
            .iter()
            .find(|candidate| candidate.candidate_id == "render.rejected")
            .unwrap();
        assert_eq!(
            rejected.disposition,
            CompositionCandidateDisposition::Rejected
        );
        assert!(rejected.rejection_reasons.iter().any(|reason| {
            reason.kind == CompositionRejectionKind::CapabilityVersionBelowMinimum
        }));
        assert!(rejected
            .rejection_reasons
            .iter()
            .any(|reason| { reason.kind == CompositionRejectionKind::MissingRequiredTag }));
    }

    #[test]
    fn preflight_rejection_is_preserved_by_shared_explanation_graph() {
        let candidate = candidate("render.format-mismatch", 1000).with_preflight_rejection(
            CompositionRejectionReason::new(CompositionRejectionKind::FormatMismatch)
                .with_expected(".ytd")
                .with_actual(".ydd"),
        );
        let plan = CompositionSolver::resolve([candidate]);
        assert!(plan.selected("engine.render").is_none());
        let explanation = plan.explanation().gateway("engine.render").unwrap();
        assert_eq!(
            explanation.candidates[0].disposition,
            CompositionCandidateDisposition::Rejected
        );
        assert_eq!(
            explanation.candidates[0].rejection_reasons[0].kind,
            CompositionRejectionKind::FormatMismatch
        );
    }

    #[test]
    fn parser_extracts_versioned_contract() {
        assert_eq!(
            parse_versioned_contract_id("newengine.render-provider/v12"),
            Some(("newengine.render-provider".to_owned(), 12))
        );
    }
}
