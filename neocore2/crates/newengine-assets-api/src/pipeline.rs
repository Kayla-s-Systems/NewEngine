//! Asset/world data pipeline DTOs.
//!
//! `engine.assets` owns bytes, VFS, package/source mounts, import queue,
//! dirty scan, dependency graph, UID registry, thumbnails and package writer
//! capability visibility. Semantic interpretation stays in domain gateways
//! (`engine.assets.models`, `engine.assets.materials`, `engine.assets.textures`,
//! `engine.assets.definitions`, `engine.scene`, ...). Renderers consume only
//! render-ready packets produced after those domain gateways validate data.

use serde::{Deserialize, Serialize};

pub const ASSET_PIPELINE_STATUS_SCHEMA: &str = "northstar.assets.pipeline.status.v1";
pub const ASSET_FORMAT_OWNERSHIP_SCHEMA: &str = "northstar.assets.format.ownership.v1";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AssetFormatOwnership {
    pub extension: String,
    pub role: String,
    pub byte_owner_gateway: String,
    pub semantic_owner_gateway: String,
    pub runtime_rule: String,
}

impl Default for AssetFormatOwnership {
    fn default() -> Self {
        Self {
            extension: String::new(),
            role: String::new(),
            byte_owner_gateway: crate::ENGINE_ASSET_SERVICE_ID.to_owned(),
            semantic_owner_gateway: String::new(),
            runtime_rule: String::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AssetPipelineStatusV1 {
    pub schema: String,
    pub vfs_online: bool,
    pub packages_online: bool,
    pub source_mounts_online: bool,
    pub import_queue_online: bool,
    pub dirty_scan_online: bool,
    pub dependency_graph_online: bool,
    pub uid_registry_online: bool,
    pub thumbnails_online: bool,
    pub package_writer_online: bool,
    pub warnings: Vec<String>,
}

impl Default for AssetPipelineStatusV1 {
    fn default() -> Self {
        Self {
            schema: ASSET_PIPELINE_STATUS_SCHEMA.to_owned(),
            vfs_online: false,
            packages_online: false,
            source_mounts_online: false,
            import_queue_online: false,
            dirty_scan_online: false,
            dependency_graph_online: false,
            uid_registry_online: false,
            thumbnails_online: false,
            package_writer_online: false,
            warnings: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AssetPipelineSnapshotV1 {
    pub schema: String,
    pub ownership: Vec<AssetFormatOwnership>,
    pub status: AssetPipelineStatusV1,
}

impl Default for AssetPipelineSnapshotV1 {
    fn default() -> Self {
        Self {
            schema: ASSET_PIPELINE_STATUS_SCHEMA.to_owned(),
            ownership: canonical_asset_format_ownership(),
            status: AssetPipelineStatusV1::default(),
        }
    }
}

pub fn canonical_asset_format_ownership() -> Vec<AssetFormatOwnership> {
    vec![
        AssetFormatOwnership {
            extension: "ytyp".to_owned(),
            role: "archetype_dictionary".to_owned(),
            byte_owner_gateway: crate::ENGINE_ASSET_SERVICE_ID.to_owned(),
            semantic_owner_gateway: crate::ENGINE_ASSETS_DEFINITIONS_SERVICE_ID.to_owned(),
            runtime_rule: ".ytyp is a NEF8/ListFile definition dictionary; scene/world consume validated DTOs".to_owned(),
        },
        AssetFormatOwnership {
            extension: "ydd".to_owned(),
            role: "drawable_dictionary".to_owned(),
            byte_owner_gateway: crate::ENGINE_ASSET_SERVICE_ID.to_owned(),
            semantic_owner_gateway: crate::ENGINE_ASSETS_MODELS_SERVICE_ID.to_owned(),
            runtime_rule: ".ydd is a NEF8/ListFile drawable dictionary; renderer never parses it directly".to_owned(),
        },
        AssetFormatOwnership {
            extension: "nemat".to_owned(),
            role: "material_library".to_owned(),
            byte_owner_gateway: crate::ENGINE_ASSET_SERVICE_ID.to_owned(),
            semantic_owner_gateway: crate::ENGINE_ASSETS_MATERIALS_SERVICE_ID.to_owned(),
            runtime_rule: ".nemat is a NEF8/ListFile material library; render receives render-ready material packets".to_owned(),
        },
        AssetFormatOwnership {
            extension: "ytd".to_owned(),
            role: "texture_dictionary".to_owned(),
            byte_owner_gateway: crate::ENGINE_ASSET_SERVICE_ID.to_owned(),
            semantic_owner_gateway: crate::ENGINE_ASSETS_TEXTURES_SERVICE_ID.to_owned(),
            runtime_rule: ".ytd is a NEF8/ListFile texture dictionary; render receives runtime texture packets".to_owned(),
        },
        AssetFormatOwnership {
            extension: "nepak".to_owned(),
            role: "vfs_package".to_owned(),
            byte_owner_gateway: crate::ENGINE_ASSET_SERVICE_ID.to_owned(),
            semantic_owner_gateway: crate::ENGINE_ASSET_SERVICE_ID.to_owned(),
            runtime_rule: ".nepak is a mounted VFS package, not a ListFile".to_owned(),
        },
    ]
}


pub const ASSET_IMPORTER_DESCRIPTOR_SCHEMA: &str = "northstar.assets.importer_descriptor.v1";
pub const ASSET_RUNTIME_GRAPH_SCHEMA: &str = "northstar.assets.runtime_graph.v1";
pub const ASSET_INVALIDATION_PLAN_SCHEMA: &str = "northstar.assets.invalidation_plan.v1";
pub const ASSET_CACHE_KEY_SCHEMA: &str = "northstar.assets.cache_key.v1";
pub const NEPAK_PACKAGE_WRITER_CAPABILITY_ID: &str = "assets.package_writer.nepak";
pub const ASSET_PACKAGE_WRITE_NEPAK_JSON_V1: &str = "asset.package_write_nepak_json_v1";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ImporterDescriptorV1 {
    pub importer_id: String,
    pub label: String,
    pub source_extensions: Vec<String>,
    pub source_content_kinds: Vec<String>,
    pub runtime_outputs: Vec<String>,
    pub owner_gateway: String,
    pub cache_key_inputs: Vec<String>,
    pub deterministic: bool,
}

impl Default for ImporterDescriptorV1 {
    fn default() -> Self {
        Self {
            importer_id: String::new(),
            label: String::new(),
            source_extensions: Vec::new(),
            source_content_kinds: Vec::new(),
            runtime_outputs: Vec::new(),
            owner_gateway: crate::ENGINE_ASSET_SERVICE_ID.to_owned(),
            cache_key_inputs: vec![
                "source_path".to_owned(),
                "source_content_hash".to_owned(),
                "importer_id".to_owned(),
                "importer_version".to_owned(),
                "settings_hash".to_owned(),
            ],
            deterministic: true,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AssetCacheKeyV1 {
    pub schema: String,
    pub source_ref: String,
    pub source_hash: String,
    pub importer_id: String,
    pub importer_version: String,
    pub settings_hash: String,
    pub platform: String,
    pub cache_key: String,
}

impl Default for AssetCacheKeyV1 {
    fn default() -> Self {
        Self {
            schema: ASSET_CACHE_KEY_SCHEMA.to_owned(),
            source_ref: String::new(),
            source_hash: String::new(),
            importer_id: String::new(),
            importer_version: String::new(),
            settings_hash: String::new(),
            platform: "any".to_owned(),
            cache_key: String::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct SourceAssetNodeV1 {
    pub source_ref: String,
    pub content_hash: String,
    pub content_kind: String,
    pub importer_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct RuntimeAssetNodeV1 {
    pub asset_ref: String,
    pub content_kind: String,
    pub owner_gateway: String,
    pub cache_key: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct AssetDependencyEdgeV1 {
    pub from_ref: String,
    pub to_ref: String,
    pub reason: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AssetRuntimeGraphV1 {
    pub schema: String,
    pub sources: Vec<SourceAssetNodeV1>,
    pub runtime_assets: Vec<RuntimeAssetNodeV1>,
    pub dependencies: Vec<AssetDependencyEdgeV1>,
}

impl Default for AssetRuntimeGraphV1 {
    fn default() -> Self {
        Self {
            schema: ASSET_RUNTIME_GRAPH_SCHEMA.to_owned(),
            sources: Vec::new(),
            runtime_assets: Vec::new(),
            dependencies: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AssetInvalidationPlanV1 {
    pub schema: String,
    pub changed_sources: Vec<String>,
    pub invalidated_cache_keys: Vec<String>,
    pub affected_runtime_assets: Vec<String>,
    pub reason: String,
}

impl Default for AssetInvalidationPlanV1 {
    fn default() -> Self {
        Self {
            schema: ASSET_INVALIDATION_PLAN_SCHEMA.to_owned(),
            changed_sources: Vec::new(),
            invalidated_cache_keys: Vec::new(),
            affected_runtime_assets: Vec::new(),
            reason: String::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct NepakPackageWriteEntryV1 {
    pub target_path: String,
    pub source_ref: String,
    pub payload_base64: String,
    pub content_kind: String,
    pub cache_key: String,
}

impl Default for NepakPackageWriteEntryV1 {
    fn default() -> Self {
        Self {
            target_path: String::new(),
            source_ref: String::new(),
            payload_base64: String::new(),
            content_kind: String::new(),
            cache_key: String::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct NepakPackageWriteRequestV1 {
    pub package_ref: String,
    pub runtime_graph: AssetRuntimeGraphV1,
    pub entries: Vec<String>,
    pub entry_payloads: Vec<NepakPackageWriteEntryV1>,
    pub deterministic_order: bool,
    pub dry_run: bool,
    pub requested_capability: String,
}

impl Default for NepakPackageWriteRequestV1 {
    fn default() -> Self {
        Self {
            package_ref: String::new(),
            runtime_graph: AssetRuntimeGraphV1::default(),
            entries: Vec::new(),
            entry_payloads: Vec::new(),
            deterministic_order: true,
            dry_run: false,
            requested_capability: NEPAK_PACKAGE_WRITER_CAPABILITY_ID.to_owned(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct NepakPackageWriteResponseV1 {
    pub ok: bool,
    pub package_ref: String,
    pub written_entries: Vec<String>,
    pub skipped_entries: Vec<String>,
    pub package_hash: String,
    pub diagnostics: Vec<String>,
}
