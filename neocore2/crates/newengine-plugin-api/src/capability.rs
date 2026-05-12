#![forbid(unsafe_op_in_unsafe_fn)]

use abi_stable::std_types::{RString, RVec};
use abi_stable::StableAbi;

use crate::types::CapabilityId;

pub const CAPABILITY_TAG_LEGACY: &str = "legacy";
pub const CAPABILITY_TAG_RENDER: &str = "render";
pub const CAPABILITY_TAG_RUNTIME: &str = "runtime";
pub const CAPABILITY_ID_RENDER_DRAW_LIST_PROVIDER_V1: &str = "render.draw_list_provider.v1";
pub const CAPABILITY_ID_RENDER_LIGHT_EXTRACTION_PROVIDER_V1: &str = "render.light_extraction_provider.v1";

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
    capability_json_has_tag(capability.describe_json.as_str(), tag)
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
