#![forbid(unsafe_op_in_unsafe_fn)]

use abi_stable::std_types::{ROption, RString, RVec};
use abi_stable::StableAbi;

use crate::types::CapabilityId;

pub use newengine_service_api::{BackendRouteDescriptor, BackendServiceSpec};

pub const CAPABILITY_TAG_RETIRED: &str = "retired";
pub const CAPABILITY_TAG_RENDER: &str = "render";
pub const CAPABILITY_TAG_RUNTIME: &str = "runtime";
/// Marks capabilities that enable live authoring/editing tooling.
pub const CAPABILITY_TAG_EDITING: &str = "editing";
/// Optional capability that makes editor/live-authoring tools available over the current runtime world.
pub const CAPABILITY_ID_EDITING_TOOLS: &str = "engine.editing.tools";
pub const CAPABILITY_ID_RENDER_DRAW_LIST_PROVIDER: &str = "render.draw_list_provider";
pub const CAPABILITY_ID_RENDER_LIGHT_EXTRACTION_PROVIDER: &str = "render.light_extraction_provider";
/// Optional importer that turns opaque SpeedTree .srt sources into engine runtime assets.
pub const CAPABILITY_ID_FOLIAGE_SRT_IMPORTER: &str = "assets.models.foliage.srt_importer";
/// Optional importer that turns opaque SpeedTree Modeler .spm sources into engine runtime assets.
pub const CAPABILITY_ID_FOLIAGE_SPM_IMPORTER: &str = "assets.models.foliage.spm_importer";
/// Optional GPU culling/indirect adapter. Generic CPU foliage extraction is mandatory.
pub const CAPABILITY_ID_RENDER_FOLIAGE_GPU_CULLING: &str = "render.foliage.gpu_culling";

#[inline]
pub fn capability_json_has_tag(describe_json: &str, tag: &str) -> bool {
    let tag = tag.trim();
    if tag.is_empty() {
        return false;
    }

    let compact = describe_json
        .chars()
        .filter(|ch| !ch.is_ascii_whitespace())
        .collect::<String>();
    if !compact.contains("\"tags\":[") {
        return false;
    }

    let quoted = format!("\"{}\"", tag);
    compact
        .split("\"tags\":[")
        .skip(1)
        .filter_map(|tail| tail.split(']').next())
        .any(|array| array.split(',').any(|entry| entry == quoted))
}

#[inline]
pub fn capability_has_tag(capability: &CapabilityDesc, tag: &str) -> bool {
    CapabilityDescV2::from_legacy(capability).has_tag(tag)
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, StableAbi)]
pub enum PluginKind {
    Runtime = 1,
    Importer = 2,
    Editor = 3,
    Tool = 4,
    Other = 255,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, StableAbi)]
pub enum CapabilityRole {
    Provides = 1,
    Requires = 2,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, StableAbi)]
pub enum CapabilityKind {
    ServiceV1 = 1,
    EventsV1 = 2,
    AssetImporterV1 = 3,
    SceneContributionV1 = 4,
    Other = 255,
}

/// Stable ABI contract reference used by typed capability metadata.
/// `version = RNone` means that a legacy descriptor named the contract but did
/// not provide an ABI contract version. New V2 producers should set it.
#[repr(C)]
#[derive(Debug, Clone, PartialEq, Eq, StableAbi)]
pub struct ContractRefV2 {
    pub id: RString,
    pub version: ROption<u32>,
}

impl ContractRefV2 {
    #[inline]
    pub fn new(id: impl Into<RString>, version: u32) -> Self {
        Self {
            id: id.into(),
            version: ROption::RSome(version),
        }
    }

    #[inline]
    pub fn unversioned(id: impl Into<RString>) -> Self {
        Self {
            id: id.into(),
            version: ROption::RNone,
        }
    }
}

/// Stable ABI system/composition tag. Unlike `newengine_service_api::SystemTag`,
/// this representation owns its string and is safe across a dynamic-library ABI.
#[repr(C)]
#[derive(Debug, Clone, PartialEq, Eq, StableAbi)]
pub struct SystemTagV2(pub RString);

impl SystemTagV2 {
    #[inline]
    pub fn new(value: impl Into<RString>) -> Self {
        Self(value.into())
    }

    #[inline]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

/// Typed provider-route metadata consumed by composition/routing.
/// JSON is deliberately excluded from route-critical fields.
#[repr(C)]
#[derive(Debug, Clone, PartialEq, Eq, StableAbi)]
pub struct BackendRouteDescriptorV2 {
    pub service_kind: RString,
    pub engine_gateway: RString,
    pub provider_service_id: RString,
    pub provider_abi: ROption<RString>,
    pub provider_route: ROption<RString>,
    pub backend_priority: i32,
    pub backend: ROption<RString>,
    pub mode: ROption<RString>,
    pub features: RVec<RString>,
}

impl BackendRouteDescriptorV2 {
    #[inline]
    pub fn new(
        service_kind: impl Into<RString>,
        engine_gateway: impl Into<RString>,
        provider_service_id: impl Into<RString>,
    ) -> Self {
        Self {
            service_kind: service_kind.into(),
            engine_gateway: engine_gateway.into(),
            provider_service_id: provider_service_id.into(),
            provider_abi: ROption::RNone,
            provider_route: ROption::RNone,
            backend_priority: 0,
            backend: ROption::RNone,
            mode: ROption::RNone,
            features: RVec::new(),
        }
    }
}

/// Typed requirement semantics for `CapabilityRole::Requires`.
#[repr(C)]
#[derive(Debug, Clone, PartialEq, Eq, StableAbi)]
pub struct CapabilityRequirementDescV2 {
    pub min_version: u32,
    pub max_version: ROption<u32>,
    pub required_tags: RVec<SystemTagV2>,
    pub preferred_tags: RVec<SystemTagV2>,
    pub forbidden_tags: RVec<SystemTagV2>,
}

impl CapabilityRequirementDescV2 {
    #[inline]
    pub fn at_least(min_version: u32) -> Self {
        Self {
            min_version,
            max_version: ROption::RNone,
            required_tags: RVec::new(),
            preferred_tags: RVec::new(),
            forbidden_tags: RVec::new(),
        }
    }

    #[inline]
    pub fn between(min_version: u32, max_version: u32) -> Self {
        let mut out = Self::at_least(min_version);
        out.max_version = ROption::RSome(max_version);
        out
    }

    #[inline]
    pub fn with_required_tag(mut self, tag: impl Into<RString>) -> Self {
        push_unique_tag(&mut self.required_tags, tag.into().as_str());
        self
    }

    #[inline]
    pub fn with_preferred_tag(mut self, tag: impl Into<RString>) -> Self {
        push_unique_tag(&mut self.preferred_tags, tag.into().as_str());
        self
    }

    #[inline]
    pub fn with_forbidden_tag(mut self, tag: impl Into<RString>) -> Self {
        push_unique_tag(&mut self.forbidden_tags, tag.into().as_str());
        self
    }
}

/// V2 capability metadata: all semantics used by composition, routing and
/// validation are typed. `extension_json` is reserved for domain-specific data
/// that does not participate in generic host decisions.
#[repr(C)]
#[derive(Debug, Clone, StableAbi)]
pub struct CapabilityDescV2 {
    pub id: CapabilityId,
    pub role: CapabilityRole,
    pub kind: CapabilityKind,
    pub version: u32,
    pub contract: ROption<ContractRefV2>,
    pub tags: RVec<SystemTagV2>,
    pub route: ROption<BackendRouteDescriptorV2>,
    pub requirement: ROption<CapabilityRequirementDescV2>,
    pub extension_json: RString,
}

impl CapabilityDescV2 {
    #[inline]
    pub fn new(
        id: impl Into<CapabilityId>,
        role: CapabilityRole,
        kind: CapabilityKind,
        version: u32,
    ) -> Self {
        Self {
            id: id.into(),
            role,
            kind,
            version,
            contract: ROption::RNone,
            tags: RVec::new(),
            route: ROption::RNone,
            requirement: if role == CapabilityRole::Requires {
                ROption::RSome(CapabilityRequirementDescV2::at_least(version))
            } else {
                ROption::RNone
            },
            extension_json: RString::new(),
        }
    }

    #[inline]
    pub fn with_contract(mut self, contract: ContractRefV2) -> Self {
        self.contract = ROption::RSome(contract);
        self
    }

    #[inline]
    pub fn with_requirement(mut self, requirement: CapabilityRequirementDescV2) -> Self {
        self.requirement = ROption::RSome(requirement);
        self
    }

    #[inline]
    pub fn with_tag(mut self, tag: impl Into<RString>) -> Self {
        let tag = SystemTagV2::new(tag);
        if !self
            .tags
            .iter()
            .any(|existing| existing.as_str() == tag.as_str())
        {
            self.tags.push(tag);
        }
        self
    }

    #[inline]
    pub fn with_route(mut self, route: BackendRouteDescriptorV2) -> Self {
        self.route = ROption::RSome(route);
        self
    }

    #[inline]
    pub fn with_extension_json(mut self, json: impl Into<RString>) -> Self {
        self.extension_json = json.into();
        self
    }

    #[inline]
    pub fn has_tag(&self, tag: &str) -> bool {
        let tag = tag.trim();
        !tag.is_empty() && self.tags.iter().any(|candidate| candidate.as_str() == tag)
    }

    /// Builds typed V2 route metadata without serializing routing semantics to JSON.
    pub fn backend_route(
        id: impl Into<CapabilityId>,
        version: u32,
        descriptor: BackendRouteDescriptor,
    ) -> Self {
        let mut tags = RVec::new();
        for tag in descriptor.system_tags.iter().copied() {
            push_unique_tag(&mut tags, tag);
        }
        if let Some(backend) = descriptor.backend {
            push_unique_tag(
                &mut tags,
                &format!("backend.{}", metadata_tag_slug(backend)),
            );
        }
        if let Some(mode) = descriptor.mode {
            push_unique_tag(&mut tags, &format!("mode.{}", metadata_tag_slug(mode)));
        }
        for feature in descriptor.features.iter().copied() {
            push_unique_tag(
                &mut tags,
                &format!("feature.{}", metadata_tag_slug(feature)),
            );
        }
        let extension_json = if descriptor.metadata.is_empty() {
            RString::new()
        } else {
            RString::from(serde_json::to_string(&descriptor.metadata).unwrap_or_default())
        };
        let route = BackendRouteDescriptorV2 {
            service_kind: RString::from(descriptor.service_kind),
            engine_gateway: RString::from(descriptor.engine_gateway),
            provider_service_id: RString::from(descriptor.contract),
            provider_abi: descriptor.provider_abi.map(RString::from).into(),
            provider_route: descriptor.provider_route.map(RString::from).into(),
            backend_priority: descriptor.backend_priority,
            backend: descriptor.backend.map(RString::from).into(),
            mode: descriptor.mode.map(RString::from).into(),
            features: descriptor
                .features
                .into_iter()
                .map(RString::from)
                .collect::<Vec<_>>()
                .into(),
        };
        Self {
            id: id.into(),
            role: CapabilityRole::Provides,
            kind: CapabilityKind::Other,
            version,
            contract: ROption::RSome(ContractRefV2::unversioned(
                route.provider_service_id.clone(),
            )),
            tags,
            route: ROption::RSome(route),
            requirement: ROption::RNone,
            extension_json,
        }
    }

    /// Canonical V1 -> V2 compatibility normalization. Generic host code should
    /// call this instead of parsing `describe_json` itself.
    pub fn from_legacy(capability: &CapabilityDesc) -> Self {
        let mut out = Self::new(
            capability.id.clone(),
            capability.role,
            capability.kind,
            capability.version,
        );
        let raw = capability.describe_json.as_str().trim();
        if raw.is_empty() {
            return out;
        }
        out.extension_json = capability.describe_json.clone();
        let Ok(value) = serde_json::from_str::<serde_json::Value>(raw) else {
            return out;
        };

        if let Some(contract) = json_string(&value, "contract") {
            out.contract = ROption::RSome(ContractRefV2::unversioned(contract));
        }

        collect_json_tags(&value, "tags", &mut out.tags);
        collect_json_tags(&value, "system_tags", &mut out.tags);
        if let Some(backend) = json_string(&value, "backend") {
            push_unique_tag(
                &mut out.tags,
                &format!("backend.{}", metadata_tag_slug(&backend)),
            );
        }
        if let Some(mode) = json_string(&value, "mode") {
            push_unique_tag(&mut out.tags, &format!("mode.{}", metadata_tag_slug(&mode)));
        }
        match value.get("features") {
            Some(serde_json::Value::Array(values)) => {
                for feature in values.iter().filter_map(serde_json::Value::as_str) {
                    push_unique_tag(
                        &mut out.tags,
                        &format!("feature.{}", metadata_tag_slug(feature)),
                    );
                }
            }
            Some(serde_json::Value::String(feature)) => {
                push_unique_tag(
                    &mut out.tags,
                    &format!("feature.{}", metadata_tag_slug(feature)),
                );
            }
            _ => {}
        }

        let gateway = json_string(&value, "engine_gateway");
        let service_kind = json_string(&value, "service_kind");
        if let (Some(engine_gateway), Some(service_kind)) = (gateway, service_kind) {
            let provider_service_id = json_string(&value, "contract").unwrap_or_default();
            out.route = ROption::RSome(BackendRouteDescriptorV2 {
                service_kind: service_kind.into(),
                engine_gateway: engine_gateway.into(),
                provider_service_id: provider_service_id.into(),
                provider_abi: json_string(&value, "provider_abi")
                    .map(RString::from)
                    .into(),
                provider_route: json_string(&value, "provider_route")
                    .map(RString::from)
                    .into(),
                backend_priority: value
                    .get("backend_priority")
                    .and_then(serde_json::Value::as_i64)
                    .unwrap_or(0)
                    .clamp(i32::MIN as i64, i32::MAX as i64)
                    as i32,
                backend: json_string(&value, "backend").map(RString::from).into(),
                mode: json_string(&value, "mode").map(RString::from).into(),
                features: json_strings(&value, "features")
                    .into_iter()
                    .map(RString::from)
                    .collect::<Vec<_>>()
                    .into(),
            });
        }
        out
    }
}

#[repr(C)]
#[derive(Debug, Clone, StableAbi)]
pub struct PluginDescriptorV2 {
    pub id: RString,
    pub name: RString,
    pub version: RString,
    pub kind: PluginKind,
    pub capabilities: RVec<CapabilityDescV2>,
    pub extension_json: RString,
}

impl PluginDescriptorV2 {
    pub fn from_legacy(descriptor: &PluginDescriptor) -> Self {
        Self {
            id: descriptor.id.clone(),
            name: descriptor.name.clone(),
            version: descriptor.version.clone(),
            kind: descriptor.kind,
            capabilities: descriptor
                .capabilities
                .iter()
                .map(CapabilityDescV2::from_legacy)
                .collect::<Vec<_>>()
                .into(),
            extension_json: RString::new(),
        }
    }
}

fn json_string(value: &serde_json::Value, field: &str) -> Option<String> {
    value
        .get(field)
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

fn json_strings(value: &serde_json::Value, field: &str) -> Vec<String> {
    match value.get(field) {
        Some(serde_json::Value::Array(values)) => values
            .iter()
            .filter_map(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_owned)
            .collect(),
        Some(serde_json::Value::String(value)) if !value.trim().is_empty() => {
            vec![value.trim().to_owned()]
        }
        _ => Vec::new(),
    }
}

fn collect_json_tags(value: &serde_json::Value, field: &str, out: &mut RVec<SystemTagV2>) {
    for tag in json_strings(value, field) {
        push_unique_tag(out, &tag);
    }
}

fn push_unique_tag(out: &mut RVec<SystemTagV2>, tag: &str) {
    let tag = tag.trim();
    if tag.is_empty() || out.iter().any(|candidate| candidate.as_str() == tag) {
        return;
    }
    out.push(SystemTagV2::new(tag));
}

fn metadata_tag_slug(value: &str) -> String {
    value
        .trim()
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '.'
            }
        })
        .collect::<String>()
        .split('.')
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>()
        .join(".")
}

#[repr(C)]
#[derive(Debug, Clone, StableAbi)]
pub struct CapabilityDesc {
    pub id: CapabilityId,
    pub role: CapabilityRole,
    pub kind: CapabilityKind,
    pub version: u32,
    pub describe_json: RString,
}

impl CapabilityDesc {
    #[inline]
    pub fn new(
        id: impl Into<CapabilityId>,
        role: CapabilityRole,
        kind: CapabilityKind,
        version: u32,
    ) -> Self {
        Self {
            id: id.into(),
            role,
            kind,
            version,
            describe_json: RString::new(),
        }
    }

    #[inline]
    pub fn with_json(mut self, json: impl Into<RString>) -> Self {
        self.describe_json = json.into();
        self
    }

    #[inline]
    pub fn backend_route(id: impl Into<CapabilityId>, descriptor: BackendRouteDescriptor) -> Self {
        Self::new(id, CapabilityRole::Provides, CapabilityKind::Other, 1)
            .with_json(descriptor.to_json_string())
    }

    #[inline]
    pub fn with_backend_route(mut self, descriptor: BackendRouteDescriptor) -> Self {
        self.describe_json = RString::from(descriptor.to_json_string());
        self
    }

    #[inline]
    pub fn to_v2_compat(&self) -> CapabilityDescV2 {
        CapabilityDescV2::from_legacy(self)
    }

    #[inline]
    pub fn has_tag(&self, tag: &str) -> bool {
        capability_has_tag(self, tag)
    }
}

#[repr(C)]
#[derive(Debug, Clone, StableAbi)]
pub struct PluginDescriptor {
    pub id: RString,
    pub name: RString,
    pub version: RString,
    pub kind: PluginKind,
    pub capabilities: RVec<CapabilityDesc>,
}

impl PluginDescriptor {
    #[inline]
    pub fn builder(
        id: impl Into<RString>,
        name: impl Into<RString>,
        version: impl Into<RString>,
        kind: PluginKind,
    ) -> PluginDescriptorBuilder {
        PluginDescriptorBuilder {
            id: id.into(),
            name: name.into(),
            version: version.into(),
            kind,
            capabilities: RVec::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct PluginDescriptorBuilder {
    id: RString,
    name: RString,
    version: RString,
    kind: PluginKind,
    capabilities: RVec<CapabilityDesc>,
}

impl PluginDescriptorBuilder {
    #[inline]
    pub fn push(mut self, cap: CapabilityDesc) -> Self {
        self.capabilities.push(cap);
        self
    }

    #[inline]
    pub fn provides_service(
        self,
        id: impl Into<CapabilityId>,
        version: u32,
        describe_json: impl Into<RString>,
    ) -> Self {
        self.push(
            CapabilityDesc::new(
                id,
                CapabilityRole::Provides,
                CapabilityKind::ServiceV1,
                version,
            )
            .with_json(describe_json),
        )
    }

    #[inline]
    pub fn requires_service(
        self,
        id: impl Into<CapabilityId>,
        version: u32,
        describe_json: impl Into<RString>,
    ) -> Self {
        self.push(
            CapabilityDesc::new(
                id,
                CapabilityRole::Requires,
                CapabilityKind::ServiceV1,
                version,
            )
            .with_json(describe_json),
        )
    }

    #[inline]
    pub fn provides_events(
        self,
        id: impl Into<CapabilityId>,
        version: u32,
        describe_json: impl Into<RString>,
    ) -> Self {
        self.push(
            CapabilityDesc::new(
                id,
                CapabilityRole::Provides,
                CapabilityKind::EventsV1,
                version,
            )
            .with_json(describe_json),
        )
    }

    #[inline]
    pub fn provides_asset_importer(
        self,
        id: impl Into<CapabilityId>,
        version: u32,
        describe_json: impl Into<RString>,
    ) -> Self {
        self.push(
            CapabilityDesc::new(
                id,
                CapabilityRole::Provides,
                CapabilityKind::AssetImporterV1,
                version,
            )
            .with_json(describe_json),
        )
    }

    #[inline]
    pub fn provides_scene_contribution(
        self,
        id: impl Into<CapabilityId>,
        version: u32,
        describe_json: impl Into<RString>,
    ) -> Self {
        self.push(
            CapabilityDesc::new(
                id,
                CapabilityRole::Provides,
                CapabilityKind::SceneContributionV1,
                version,
            )
            .with_json(describe_json),
        )
    }

    #[inline]
    pub fn requires_scene_contribution(
        self,
        id: impl Into<CapabilityId>,
        version: u32,
        describe_json: impl Into<RString>,
    ) -> Self {
        self.push(
            CapabilityDesc::new(
                id,
                CapabilityRole::Requires,
                CapabilityKind::SceneContributionV1,
                version,
            )
            .with_json(describe_json),
        )
    }

    #[inline]
    pub fn build(self) -> PluginDescriptor {
        PluginDescriptor {
            id: self.id,
            name: self.name,
            version: self.version,
            kind: self.kind,
            capabilities: self.capabilities,
        }
    }
}

#[cfg(test)]
mod typed_metadata_tests {
    use super::*;
    use abi_stable::std_types::ROption;

    #[test]
    fn legacy_route_json_normalizes_once_into_typed_v2() {
        let json = serde_json::json!({
            "service_kind": "render",
            "engine_gateway": "engine.render",
            "contract": "engine.render.provider",
            "provider_route": "engine.render.vulkan",
            "provider_abi": "newengine.render-provider/v1",
            "backend_priority": 42,
            "backend": "vulkan",
            "features": ["ray-query", "mesh-shader"],
            "tags": ["provider.backend"]
        })
        .to_string();

        let legacy = CapabilityDesc::new(
            "engine.render.vulkan.backend",
            CapabilityRole::Provides,
            CapabilityKind::Other,
            3,
        )
        .with_json(json.clone());
        let typed = legacy.to_v2_compat();

        assert_eq!(typed.version, 3);
        assert!(typed.has_tag("provider.backend"));
        assert!(typed.has_tag("backend.vulkan"));
        assert!(typed.has_tag("feature.ray.query"));
        assert_eq!(typed.extension_json.as_str(), json);

        let ROption::RSome(route) = typed.route else {
            panic!("legacy gateway metadata must normalize to a typed route");
        };
        assert_eq!(route.service_kind.as_str(), "render");
        assert_eq!(route.engine_gateway.as_str(), "engine.render");
        assert_eq!(route.provider_service_id.as_str(), "engine.render.provider");
        assert_eq!(route.backend_priority, 42);
        assert_eq!(
            route.provider_abi.into_option().as_deref(),
            Some("newengine.render-provider/v1")
        );
    }

    #[test]
    fn direct_v2_route_does_not_require_json_semantics() {
        let route = BackendRouteDescriptorV2 {
            service_kind: "render".into(),
            engine_gateway: "engine.render".into(),
            provider_service_id: "engine.render.provider".into(),
            provider_abi: ROption::RSome("newengine.render-provider/v2".into()),
            provider_route: ROption::RSome("engine.render.vulkan".into()),
            backend_priority: 100,
            backend: ROption::RSome("vulkan".into()),
            mode: ROption::RNone,
            features: vec!["mesh-shader".into()].into(),
        };
        let typed = CapabilityDescV2::new(
            "engine.render.vulkan.backend",
            CapabilityRole::Provides,
            CapabilityKind::Other,
            7,
        )
        .with_contract(ContractRefV2::new("engine.render.provider", 2))
        .with_tag("provider.backend")
        .with_route(route);

        assert!(typed.extension_json.is_empty());
        assert_eq!(typed.version, 7);
        assert!(typed.has_tag("provider.backend"));
        let ROption::RSome(contract) = typed.contract else {
            panic!("typed contract is required");
        };
        assert_eq!(contract.id.as_str(), "engine.render.provider");
        assert_eq!(contract.version.into_option(), Some(2));
    }

    #[test]
    fn malformed_legacy_extension_does_not_create_typed_route() {
        let legacy = CapabilityDesc::new(
            "broken.metadata",
            CapabilityRole::Provides,
            CapabilityKind::Other,
            1,
        )
        .with_json("{not-json");
        let typed = legacy.to_v2_compat();
        assert!(matches!(typed.route, ROption::RNone));
        assert_eq!(typed.extension_json.as_str(), "{not-json");
    }
}
