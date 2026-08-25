/// Stable logical capability identity used by composition requirements.
///
/// The public identity (`id`) is what products such as GameReady ask for. The
/// gateway/service-kind binding is shared contract vocabulary and is intentionally
/// hidden from product profiles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CapabilityId {
    id: &'static str,
    gateway_id: &'static str,
    service_kind: &'static str,
}

impl CapabilityId {
    #[inline]
    pub const fn new(
        id: &'static str,
        gateway_id: &'static str,
        service_kind: &'static str,
    ) -> Self {
        Self {
            id,
            gateway_id,
            service_kind,
        }
    }

    #[inline]
    pub const fn as_str(self) -> &'static str {
        self.id
    }

    #[inline]
    pub const fn gateway_id(self) -> &'static str {
        self.gateway_id
    }

    #[inline]
    pub const fn service_kind(self) -> &'static str {
        self.service_kind
    }
}

/// Declarative system tag used by the composition solver.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SystemTag(&'static str);

impl SystemTag {
    #[inline]
    pub const fn new(value: &'static str) -> Self {
        Self(value)
    }

    #[inline]
    pub const fn as_str(self) -> &'static str {
        self.0
    }
}

/// Requirement strength for one composition capability.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RequirementStrength {
    Optional,
    Preferred,
    Required,
}

impl RequirementStrength {
    #[inline]
    pub const fn is_required(self) -> bool {
        matches!(self, Self::Required)
    }
}

/// Solver cardinality policy.
///
/// `Many` composes with requirement strength: a required-many capability resolves
/// at least one provider, while optional/preferred-many may resolve zero.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Cardinality {
    One,
    ZeroOrOne,
    Many,
}

impl Cardinality {
    #[inline]
    pub const fn min(self, strength: RequirementStrength) -> u16 {
        match self {
            Self::One => 1,
            Self::ZeroOrOne => 0,
            Self::Many if matches!(strength, RequirementStrength::Required) => 1,
            Self::Many => 0,
        }
    }

    #[inline]
    pub const fn max(self) -> u16 {
        match self {
            Self::One | Self::ZeroOrOne => 1,
            Self::Many => u16::MAX,
        }
    }
}

/// Versioned provider contract requirement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContractRequirement {
    pub contract_id: &'static str,
    pub min_version: u32,
    pub max_version: Option<u32>,
}

impl ContractRequirement {
    #[inline]
    pub const fn new(
        contract_id: &'static str,
        min_version: u32,
        max_version: Option<u32>,
    ) -> Self {
        Self {
            contract_id,
            min_version,
            max_version,
        }
    }
}

/// Explicit fallback policy for an unsatisfied primary provider set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FallbackPolicy {
    None,
    Providers(&'static [&'static str]),
}

impl FallbackPolicy {
    #[inline]
    pub const fn providers(self) -> &'static [&'static str] {
        match self {
            Self::None => &[],
            Self::Providers(providers) => providers,
        }
    }
}

/// V2 composition requirement consumed directly by the shared solver.
///
/// Product profiles describe *what* they need, not which runtime modules should
/// be registered to satisfy it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CapabilityRequirement {
    pub capability: CapabilityId,
    pub contract: Option<ContractRequirement>,
    pub strength: RequirementStrength,
    pub cardinality: Cardinality,
    /// Hard feature/tag requirements. Retained as an advanced constraint in
    /// addition to the simpler preferred/forbidden policy surface.
    pub required_tags: &'static [SystemTag],
    pub preferred_tags: &'static [SystemTag],
    pub forbidden_tags: &'static [SystemTag],
    pub fallback: FallbackPolicy,
}

impl CapabilityRequirement {
    #[inline]
    pub const fn new(capability: CapabilityId, strength: RequirementStrength) -> Self {
        Self {
            capability,
            contract: None,
            strength,
            cardinality: match strength {
                RequirementStrength::Required => Cardinality::One,
                RequirementStrength::Optional | RequirementStrength::Preferred => {
                    Cardinality::ZeroOrOne
                }
            },
            required_tags: &[],
            preferred_tags: &[],
            forbidden_tags: &[],
            fallback: FallbackPolicy::None,
        }
    }

    #[inline]
    pub const fn required(capability: CapabilityId) -> Self {
        Self::new(capability, RequirementStrength::Required)
    }

    #[inline]
    pub const fn optional(capability: CapabilityId) -> Self {
        Self::new(capability, RequirementStrength::Optional)
    }

    #[inline]
    pub const fn preferred(capability: CapabilityId) -> Self {
        Self::new(capability, RequirementStrength::Preferred)
    }

    #[inline]
    pub const fn with_contract(
        mut self,
        contract_id: &'static str,
        min_version: u32,
        max_version: Option<u32>,
    ) -> Self {
        self.contract = Some(ContractRequirement::new(
            contract_id,
            min_version,
            max_version,
        ));
        self
    }

    #[inline]
    pub const fn with_cardinality(mut self, cardinality: Cardinality) -> Self {
        self.cardinality = cardinality;
        self
    }

    #[inline]
    pub const fn with_required_tags(mut self, tags: &'static [SystemTag]) -> Self {
        self.required_tags = tags;
        self
    }

    #[inline]
    pub const fn with_preferred_tags(mut self, tags: &'static [SystemTag]) -> Self {
        self.preferred_tags = tags;
        self
    }

    #[inline]
    pub const fn with_forbidden_tags(mut self, tags: &'static [SystemTag]) -> Self {
        self.forbidden_tags = tags;
        self
    }

    #[inline]
    pub const fn with_fallback(mut self, fallback: FallbackPolicy) -> Self {
        self.fallback = fallback;
        self
    }
}

/// V1 compatibility descriptor. New composition code should use
/// `CapabilityRequirement`; this type remains only for gradual migration of old
/// profile declarations and diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EngineCapabilitySlotSpec {
    pub gateway_id: &'static str,
    pub service_kind: &'static str,
    pub required: bool,
}

impl EngineCapabilitySlotSpec {
    #[inline]
    pub const fn new(gateway_id: &'static str, service_kind: &'static str, required: bool) -> Self {
        Self {
            gateway_id,
            service_kind,
            required,
        }
    }

    #[inline]
    pub const fn required(gateway_id: &'static str, service_kind: &'static str) -> Self {
        Self::new(gateway_id, service_kind, true)
    }

    #[inline]
    pub const fn optional(gateway_id: &'static str, service_kind: &'static str) -> Self {
        Self::new(gateway_id, service_kind, false)
    }

    #[inline]
    pub const fn into_v2(self) -> CapabilityRequirement {
        let capability = CapabilityId::new(self.gateway_id, self.gateway_id, self.service_kind);
        if self.required {
            CapabilityRequirement::required(capability)
        } else {
            CapabilityRequirement::optional(capability)
        }
    }
}

// Transitional aliases for callers introduced during the first V2 migration pass.
pub type EngineCapabilityRequirementSpec = CapabilityRequirement;
pub type CapabilityRequirementLevel = RequirementStrength;
pub type CapabilityContractRequirement = ContractRequirement;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngineRuntimeUnitKind {
    Module,
    Adapter,
    Provider,
    ProductExtension,
}

/// Static runtime unit descriptor. The descriptor is pure data; the generic host
/// owns the instance-local factory catalog that materializes matching unit ids.
/// Product profiles declare capabilities only and never name implementation modules.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EngineRuntimeUnitSpec {
    pub id: &'static str,
    pub version: u32,
    pub kind: EngineRuntimeUnitKind,
    pub provides: &'static [&'static str],
    pub requires: &'static [&'static str],
    pub tags: &'static [&'static str],
}

impl EngineRuntimeUnitSpec {
    #[inline]
    pub const fn new(
        id: &'static str,
        version: u32,
        kind: EngineRuntimeUnitKind,
        provides: &'static [&'static str],
        requires: &'static [&'static str],
        tags: &'static [&'static str],
    ) -> Self {
        Self {
            id,
            version,
            kind,
            provides,
            requires,
            tags,
        }
    }
}

/// Pure data description of the engine shape requested from the host.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EngineCompositionSpec {
    pub schema_version: u32,
    pub id: &'static str,
    pub requirements: &'static [CapabilityRequirement],
    pub runtime_units: &'static [EngineRuntimeUnitSpec],
}

impl EngineCompositionSpec {
    pub const SCHEMA_VERSION: u32 = 2;

    #[inline]
    pub const fn new(id: &'static str, requirements: &'static [CapabilityRequirement]) -> Self {
        Self {
            schema_version: Self::SCHEMA_VERSION,
            id,
            requirements,
            runtime_units: &[],
        }
    }

    #[inline]
    pub const fn with_runtime_units(
        mut self,
        runtime_units: &'static [EngineRuntimeUnitSpec],
    ) -> Self {
        self.runtime_units = runtime_units;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RENDER: CapabilityId = CapabilityId::new("render.backend", "engine.render", "render");
    const AUDIO: CapabilityId = CapabilityId::new("audio.backend", "engine.audio", "audio");
    const TIMELINE: SystemTag = SystemTag::new("feature.timeline");

    const REQUIREMENTS: &[CapabilityRequirement] = &[
        CapabilityRequirement::required(RENDER)
            .with_contract("newengine.render-provider", 1, Some(2))
            .with_preferred_tags(&[TIMELINE]),
        CapabilityRequirement::preferred(AUDIO)
            .with_fallback(FallbackPolicy::Providers(&["engine.audio.null"])),
    ];

    #[test]
    fn composition_v2_is_capability_first() {
        let spec = EngineCompositionSpec::new("test.composition", REQUIREMENTS);
        assert_eq!(spec.schema_version, 2);
        assert_eq!(spec.requirements[0].capability.as_str(), "render.backend");
        assert_eq!(
            spec.requirements[0].capability.gateway_id(),
            "engine.render"
        );
        assert!(spec.requirements[0].strength.is_required());
        assert_eq!(
            spec.requirements[0]
                .contract
                .expect("render contract")
                .min_version,
            1
        );
        assert_eq!(
            spec.requirements[1].strength,
            RequirementStrength::Preferred
        );
    }

    #[test]
    fn many_cardinality_composes_with_strength() {
        assert_eq!(Cardinality::Many.min(RequirementStrength::Required), 1);
        assert_eq!(Cardinality::Many.min(RequirementStrength::Optional), 0);
        assert_eq!(Cardinality::Many.max(), u16::MAX);
    }
}
