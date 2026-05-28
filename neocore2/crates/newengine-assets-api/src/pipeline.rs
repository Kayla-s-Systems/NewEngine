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
