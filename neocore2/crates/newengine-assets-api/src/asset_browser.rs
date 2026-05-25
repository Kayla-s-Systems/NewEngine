#![forbid(unsafe_op_in_unsafe_fn)]

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

/// VFS/listFile address currently shown by Asset Browser.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AssetBrowserLocation {
    pub logical_path: String,
    pub entry: Option<String>,
    pub location_kind: String,
}

impl Default for AssetBrowserLocation {
    fn default() -> Self {
        Self { logical_path: String::new(), entry: None, location_kind: "vfs_directory".to_owned() }
    }
}

impl AssetBrowserLocation {
    #[inline]
    pub fn from_ref(reference: &str) -> Self {
        let normalized = normalize_browser_path(reference);
        if let Some((path, entry)) = normalized.split_once('@') {
            Self { logical_path: path.to_owned(), entry: Some(entry.to_owned()), location_kind: "listfile_entry".to_owned() }
        } else {
            Self { logical_path: normalized, entry: None, location_kind: "vfs_directory".to_owned() }
        }
    }

    #[inline]
    pub fn canonical_ref(&self) -> String {
        match self.entry.as_deref().map(str::trim).filter(|v| !v.is_empty()) {
            Some(entry) => format!("{}@{}", normalize_browser_path(&self.logical_path), entry),
            None => normalize_browser_path(&self.logical_path),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AssetBrowserListRequest {
    pub logical_path: String,
    pub entry: Option<String>,
    pub query: String,
    pub include_hidden: bool,
    pub include_listfile_entries: bool,
}

impl Default for AssetBrowserListRequest {
    fn default() -> Self {
        Self { logical_path: String::new(), entry: None, query: String::new(), include_hidden: false, include_listfile_entries: true }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AssetBrowserOpenRequest {
    pub target_ref: String,
    pub mode: String,
}

impl Default for AssetBrowserOpenRequest {
    fn default() -> Self { Self { target_ref: String::new(), mode: "auto".to_owned() } }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AssetBrowserNode {
    pub name: String,
    pub logical_path: String,
    pub entry_ref: Option<String>,
    pub node_kind: String,
    pub asset_kind: String,
    pub extension: Option<String>,
    pub byte_len: Option<u64>,
    pub source_kind: Option<String>,
    pub source_index: Option<usize>,
    pub mount: Option<String>,
    pub priority: Option<i32>,
    pub semantic_gateway: Option<String>,
    pub handler_service: Option<String>,
    pub route_gateway: Option<String>,
    pub route_method: Option<String>,
    pub has_children: bool,
    pub can_open: bool,
    pub can_preview: bool,
    pub can_rename: bool,
    pub can_delete: bool,
    pub can_update: bool,
    pub can_rebuild: bool,
    pub metadata: BTreeMap<String, String>,
    pub warnings: Vec<String>,
}

impl Default for AssetBrowserNode {
    fn default() -> Self {
        Self {
            name: String::new(),
            logical_path: String::new(),
            entry_ref: None,
            node_kind: "asset".to_owned(),
            asset_kind: String::new(),
            extension: None,
            byte_len: None,
            source_kind: None,
            source_index: None,
            mount: None,
            priority: None,
            semantic_gateway: None,
            handler_service: None,
            route_gateway: None,
            route_method: None,
            has_children: false,
            can_open: true,
            can_preview: true,
            can_rename: false,
            can_delete: false,
            can_update: false,
            can_rebuild: false,
            metadata: BTreeMap::new(),
            warnings: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AssetBrowserListResponse {
    pub ok: bool,
    pub schema: String,
    pub location: AssetBrowserLocation,
    pub breadcrumbs: Vec<AssetBrowserLocation>,
    pub folders: Vec<AssetBrowserNode>,
    pub assets: Vec<AssetBrowserNode>,
    pub entries: Vec<AssetBrowserNode>,
    pub sources: Vec<Value>,
    pub warnings: Vec<String>,
}

impl Default for AssetBrowserListResponse {
    fn default() -> Self {
        Self {
            ok: false,
            schema: "newengine.assets.browser.list.response.v1".to_owned(),
            location: AssetBrowserLocation::default(),
            breadcrumbs: Vec::new(),
            folders: Vec::new(),
            assets: Vec::new(),
            entries: Vec::new(),
            sources: Vec::new(),
            warnings: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AssetBrowserSnapshotResponse {
    pub ok: bool,
    pub schema: String,
    pub root: AssetBrowserListResponse,
    pub sources: Vec<Value>,
    pub file_type_manifest: Value,
    pub formats: Value,
    pub warnings: Vec<String>,
}

impl Default for AssetBrowserSnapshotResponse {
    fn default() -> Self {
        Self {
            ok: false,
            schema: "newengine.assets.browser.snapshot.response.v1".to_owned(),
            root: AssetBrowserListResponse::default(),
            sources: Vec::new(),
            file_type_manifest: Value::Null,
            formats: Value::Null,
            warnings: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AssetBrowserEntryMutationRequest {
    pub target_ref: String,
    pub operation: String,
    pub new_name: Option<String>,
    pub payload_base64: Option<String>,
    pub payload_json: Option<Value>,
    pub dry_run: bool,
}

impl Default for AssetBrowserEntryMutationRequest {
    fn default() -> Self {
        Self { target_ref: String::new(), operation: String::new(), new_name: None, payload_base64: None, payload_json: None, dry_run: false }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AssetBrowserRebuildRequest {
    pub logical_path: String,
    pub dry_run: bool,
    pub verify_after_build: bool,
}

impl Default for AssetBrowserRebuildRequest {
    fn default() -> Self { Self { logical_path: String::new(), dry_run: false, verify_after_build: true } }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AssetBrowserMutationResponse {
    pub ok: bool,
    pub accepted: bool,
    pub applied: bool,
    pub schema: String,
    pub target_ref: String,
    pub logical_path: String,
    pub entry: Option<String>,
    pub operation: String,
    pub transaction_id: String,
    pub message: String,
    pub warnings: Vec<String>,
    pub required_capability: Option<String>,
    pub bytes_written: Option<u64>,
    pub body_raw_hash_before: Option<String>,
    pub body_raw_hash_after: Option<String>,
    pub source_kind: Option<String>,
    pub mount: Option<String>,
    pub listfile_manifest: Value,
}

impl Default for AssetBrowserMutationResponse {
    fn default() -> Self {
        Self {
            ok: false,
            accepted: false,
            applied: false,
            schema: "newengine.assets.browser.mutation.response.v1".to_owned(),
            target_ref: String::new(),
            logical_path: String::new(),
            entry: None,
            operation: String::new(),
            transaction_id: String::new(),
            message: String::new(),
            warnings: Vec::new(),
            required_capability: None,
            bytes_written: None,
            body_raw_hash_before: None,
            body_raw_hash_after: None,
            source_kind: None,
            mount: None,
            listfile_manifest: Value::Null,
        }
    }
}

#[inline]
pub fn normalize_browser_path(value: &str) -> String {
    let mut out = value.trim().replace('\\', "/");
    while let Some(rest) = out.strip_prefix("./") {
        out = rest.to_owned();
    }
    out = out.trim_start_matches('/').to_owned();
    while out.contains("//") {
        out = out.replace("//", "/");
    }
    out
}
