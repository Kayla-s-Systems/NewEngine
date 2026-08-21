use super::*;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct AssetGraphResolveRequest {
    pub root_ref: String,
}

impl AssetGraphResolveRequest {
    #[inline]
    pub fn root(&self) -> &str {
        self.root_ref.trim()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AssetGraphCacheKeyParts {
    pub logical_path: String,
    pub entry: Option<String>,
    pub content_hash: Option<String>,
    pub schema_version: String,
    pub import_settings_hash: Option<String>,
    pub provider_version: Option<String>,
}
impl Default for AssetGraphCacheKeyParts {
    fn default() -> Self {
        Self {
            logical_path: String::new(),
            entry: None,
            content_hash: None,
            schema_version: "v2".to_owned(),
            import_settings_hash: None,
            provider_version: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AssetGraphVfsSource {
    pub source_kind: String,
    pub logical_path: String,
    pub physical_path: Option<String>,
    pub package_path: Option<String>,
    pub package_entry: Option<String>,
    pub layer_id: Option<String>,
    pub overridden_by: Vec<String>,
}
impl Default for AssetGraphVfsSource {
    fn default() -> Self {
        Self {
            source_kind: "unresolved".to_owned(),
            logical_path: String::new(),
            physical_path: None,
            package_path: None,
            package_entry: None,
            layer_id: None,
            overridden_by: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AssetGraphNode {
    pub id: String,
    pub reference: String,
    #[serde(rename = "ref")]
    pub asset_ref: String,
    pub role: String,
    pub kind: String,
    pub asset_kind: String,
    pub byte_owner: String,
    pub semantic_gateway: String,
    pub method: String,
    pub semantic_owner: String,
    pub vfs_source: AssetGraphVfsSource,
    pub content_hash: Option<String>,
    pub entry_hash: Option<String>,
    pub schema_version: String,
    pub cache_key_parts: AssetGraphCacheKeyParts,
    pub metadata_namespaces: Vec<String>,
    pub warnings: Vec<String>,
}
impl Default for AssetGraphNode {
    fn default() -> Self {
        Self {
            id: String::new(),
            reference: String::new(),
            asset_ref: String::new(),
            role: String::new(),
            kind: String::new(),
            asset_kind: String::new(),
            byte_owner: "engine.assets".to_owned(),
            semantic_gateway: String::new(),
            method: String::new(),
            semantic_owner: String::new(),
            vfs_source: AssetGraphVfsSource::default(),
            content_hash: None,
            entry_hash: None,
            schema_version: "v2".to_owned(),
            cache_key_parts: AssetGraphCacheKeyParts::default(),
            metadata_namespaces: Vec::new(),
            warnings: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AssetGraphEdge {
    pub from: String,
    pub to: String,
    pub from_ref: String,
    pub to_ref: String,
    pub kind: String,
    pub required: bool,
}
impl Default for AssetGraphEdge {
    fn default() -> Self {
        Self {
            from: String::new(),
            to: String::new(),
            from_ref: String::new(),
            to_ref: String::new(),
            kind: String::new(),
            required: true,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ResolvedAssetGraphV1 {
    pub schema: String,
    pub root_ref: String,
    pub source: String,
    pub nodes: Vec<AssetGraphNode>,
    pub edges: Vec<AssetGraphEdge>,
    pub missing_refs: Vec<String>,
    pub cycle_errors: Vec<String>,
    pub format_warnings: Vec<String>,
    pub metadata_warnings: Vec<String>,
    pub migration_warnings: Vec<String>,
    pub cache_key_parts: AssetGraphCacheKeyParts,
    pub node_cache_key_parts: Vec<AssetGraphCacheKeyParts>,
    pub stable_cache_key: String,
    pub cache_key_policy: String,
    pub debug_log: Vec<String>,
}
impl Default for ResolvedAssetGraphV1 {
    fn default() -> Self {
        Self {
            schema: ASSET_GRAPH_SCHEMA.to_owned(),
            root_ref: String::new(),
            source: String::new(),
            nodes: Vec::new(),
            edges: Vec::new(),
            missing_refs: Vec::new(),
            cycle_errors: Vec::new(),
            format_warnings: Vec::new(),
            metadata_warnings: Vec::new(),
            migration_warnings: Vec::new(),
            cache_key_parts: AssetGraphCacheKeyParts::default(),
            node_cache_key_parts: Vec::new(),
            stable_cache_key: String::new(),
            cache_key_policy: "graph(root_ref + ordered nodes + ordered edges + content_hash + entry_hash + schema_version + provider_version)".to_owned(),
            debug_log: Vec::new(),
        }
    }
}

pub type ResolvedAssetGraphV2 = ResolvedAssetGraphV1;
pub type ResolvedAssetGraph = ResolvedAssetGraphV2;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct AssetGraphValidationResult {
    pub valid: bool,
    pub root_ref: String,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
    pub graph: Option<ResolvedAssetGraphV2>,
}
