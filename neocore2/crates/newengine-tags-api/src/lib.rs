#![forbid(unsafe_op_in_unsafe_fn)]

//! Stable DTO contract for the `engine.tags` gateway.
//!
//! Tags are the common gameplay vocabulary consumed by AI, tasks, animation,
//! interaction, scripting and debug tools. They are data declarations, not
//! hardcoded gameplay branches.

use newengine_entity_api::EntityHandle;
use serde::{Deserialize, Serialize};

pub const ENGINE_TAGS_SERVICE_ID: &str = "engine.tags";
pub const TAGS_SERVICE_ID: &str = "tags.api";
pub const TAGS_REGISTRY_CAPABILITY_ID: &str = "tags.registry";
pub const TAGS_RUNTIME_CONTRACT: &str = "newengine.tags-api/v1";

pub mod tags_method {
    pub const INFO_JSON: &str = newengine_service_api::SERVICE_METHOD_INFO_JSON;
    pub const INVOKE_JSON: &str = newengine_service_api::SERVICE_METHOD_INVOKE_JSON;
    pub const SHUTDOWN_V1: &str = newengine_service_api::SERVICE_METHOD_SHUTDOWN_V1;
    pub const DESCRIBE_TAGS_JSON_V1: &str = "tags.describe_tags_json_v1";
    pub const RESOLVE_TAG_JSON_V1: &str = "tags.resolve_tag_json_v1";
    pub const SNAPSHOT_JSON_V1: &str = "tags.snapshot_json_v1";
    pub const VALIDATE_TAG_SET_JSON_V1: &str = "tags.validate_tag_set_json_v1";
}

pub const TAGS_SERVICE_METHODS: &[&str] = &[
    tags_method::INFO_JSON,
    tags_method::INVOKE_JSON,
    tags_method::SHUTDOWN_V1,
    tags_method::DESCRIBE_TAGS_JSON_V1,
    tags_method::RESOLVE_TAG_JSON_V1,
    tags_method::SNAPSHOT_JSON_V1,
    tags_method::VALIDATE_TAG_SET_JSON_V1,
];

pub const TAGS_BACKEND_SERVICE_SPEC: newengine_service_api::BackendServiceSpec =
    newengine_service_api::BackendServiceSpec::new(
        "tags",
        ENGINE_TAGS_SERVICE_ID,
        TAGS_SERVICE_ID,
        TAGS_REGISTRY_CAPABILITY_ID,
    );

pub const TAGS_RUNTIME_CONTRACT_SPEC: newengine_service_api::RuntimeServiceContractSpec =
    newengine_service_api::RuntimeServiceContractSpec::new(
        ENGINE_TAGS_SERVICE_ID,
        "newengine.tags-api >= 0.1.x",
        TAGS_SERVICE_METHODS,
    );

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct TagId(pub String);

impl TagId {
    #[inline]
    pub fn new(value: impl Into<String>) -> Self { Self(value.into()) }

    #[inline]
    pub fn as_str(&self) -> &str { &self.0 }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TagDomain {
    Gameplay,
    State,
    Faction,
    Item,
    Weapon,
    Mission,
    Animation,
    Navigation,
    Debug,
    Custom(String),
}

impl Default for TagDomain {
    #[inline]
    fn default() -> Self { Self::Gameplay }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TagDescriptorV1 {
    pub tag: TagId,
    #[serde(default)]
    pub domain: TagDomain,
    #[serde(default)]
    pub display_name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub parent: Option<TagId>,
    #[serde(default)]
    pub aliases: Vec<TagId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct TagSetSnapshotV1 {
    pub owner: String,
    #[serde(default)]
    pub entity: Option<EntityHandle>,
    #[serde(default)]
    pub tags: Vec<TagId>,
    #[serde(default)]
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct TagsDescribeRequestV1 {
    #[serde(default)]
    pub domain_filter: Option<String>,
    #[serde(default)]
    pub include_aliases: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct TagsDescribeResponseV1 {
    pub accepted: bool,
    #[serde(default)]
    pub tags: Vec<TagDescriptorV1>,
    #[serde(default)]
    pub diagnostics: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct TagsResolveRequestV1 {
    pub tag: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct TagsResolveResponseV1 {
    pub accepted: bool,
    #[serde(default)]
    pub descriptor: Option<TagDescriptorV1>,
    #[serde(default)]
    pub diagnostics: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct TagsSnapshotRequestV1 {
    #[serde(default)]
    pub owner_prefix: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct TagsSnapshotResponseV1 {
    pub accepted: bool,
    #[serde(default)]
    pub sets: Vec<TagSetSnapshotV1>,
    #[serde(default)]
    pub diagnostics: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct TagsValidateSetRequestV1 {
    pub owner: String,
    #[serde(default)]
    pub tags: Vec<TagId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct TagsValidateSetResponseV1 {
    pub accepted: bool,
    #[serde(default)]
    pub normalized_tags: Vec<TagId>,
    #[serde(default)]
    pub unknown_tags: Vec<TagId>,
    #[serde(default)]
    pub diagnostics: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TagsServiceInfoV1 {
    pub protocol: String,
    pub provider: String,
    #[serde(default)]
    pub methods: Vec<String>,
    #[serde(default)]
    pub features: Vec<String>,
}

impl Default for TagsServiceInfoV1 {
    fn default() -> Self {
        Self {
            protocol: TAGS_RUNTIME_CONTRACT.to_owned(),
            provider: "engine.tags.foundation".to_owned(),
            methods: TAGS_SERVICE_METHODS.iter().map(|it| (*it).to_owned()).collect(),
            features: vec!["gameplay-vocabulary".to_owned(), "data-driven-tags".to_owned()],
        }
    }
}
