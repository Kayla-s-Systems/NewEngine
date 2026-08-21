use serde::{Deserialize, Serialize};

use crate::{
    DataDrivenConstructionPlan, MATERIAL_LIBRARY_ASSET_KIND, ROLE_DEFINITION_ENTRIES,
    ROLE_DRAWABLE_DICTIONARY, ROLE_MATERIAL_LIBRARY, ROLE_TEXTURE_DICTIONARY,
};

mod routing;
use routing::classify_ref;

pub const ASSET_GRAPH_SCHEMA: &str = "newengine.assets.graph.resolved.v2";
pub const ASSET_GRAPH_RESOLVED_SCHEMA_V1: &str = ASSET_GRAPH_SCHEMA;
pub const ASSET_GRAPH_RESOLVED_SCHEMA_V2: &str = ASSET_GRAPH_SCHEMA;
pub const ENGINE_ASSETS_GRAPH_SERVICE_ID: &str = "engine.assets.graph";
pub const ASSET_GRAPH_SERVICE_ID: &str = "asset_graph.api";
pub const ASSET_GRAPH_BACKEND_CAPABILITY_ID: &str = "assets.graph.backend";
pub const ASSET_GRAPH_METHOD_RESOLVE_V1: &str = "assets.graph.resolve_v1";
pub const ASSET_GRAPH_METHOD_VALIDATE_V1: &str = "assets.graph.validate_v1";
pub const ASSET_GRAPH_METHOD_DUMP_JSON_V1: &str = "assets.graph.dump_json_v1";
pub const ASSET_GRAPH_METHODS: &[&str] = &[
    newengine_service_api::SERVICE_METHOD_INFO_JSON,
    newengine_service_api::SERVICE_METHOD_INVOKE_JSON,
    newengine_service_api::SERVICE_METHOD_SHUTDOWN_V1,
    ASSET_GRAPH_METHOD_RESOLVE_V1,
    ASSET_GRAPH_METHOD_VALIDATE_V1,
    ASSET_GRAPH_METHOD_DUMP_JSON_V1,
];

mod graph_ops;
mod identity;
mod resolver;
mod types;

pub use graph_ops::{
    attach_content_hash, attach_metadata_namespace, attach_node_warning, attach_vfs_source,
    classify_asset_ref, finalize_graph, push_manifest_dependency,
};
pub use identity::{
    cache_key_parts_for_ref, fnv1a64, normalize_asset_ref, split_asset_ref, stable_graph_id,
};
pub use resolver::AssetGraphResolver;
pub use types::{
    AssetGraphCacheKeyParts, AssetGraphEdge, AssetGraphNode, AssetGraphResolveRequest,
    AssetGraphValidationResult, AssetGraphVfsSource, ResolvedAssetGraph, ResolvedAssetGraphV1,
    ResolvedAssetGraphV2,
};
