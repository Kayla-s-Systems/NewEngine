use serde::{Deserialize, Serialize};

use crate::{TagDescriptorV1, TagId, TagSetSnapshotV1};

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
