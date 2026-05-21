#![forbid(unsafe_op_in_unsafe_fn)]

use std::collections::BTreeMap;

/// Canonical manifest schema returned by dictionary/container codecs for the
/// data-driven asset world.
///
/// `listFiles` answers one generic question for every format: which addressable
/// entries does this logical file expose, and through which gateway/method should
/// those entries be resolved?
pub const ASSET_FILE_MANIFEST_SCHEMA: &str = "newengine.asset.list_files.v1";
pub const ASSET_LIST_FILE_MANIFEST_OUTPUT: &str = "asset.list_file_manifest_v1";

/// Declarative route from an asset entry to the gateway that owns its semantic
/// interpretation. AssetManager still owns VFS bytes/codec dispatch; domain
/// gateways own meaning.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct AssetGatewayRoute {
    pub gateway: String,
    pub service: String,
    pub method: String,
}

impl Default for AssetGatewayRoute {
    fn default() -> Self {
        Self { gateway: "engine.assets".to_owned(), service: "asset_manager.api".to_owned(), method: "asset.decode_v1".to_owned() }
    }
}

impl AssetGatewayRoute {
    #[inline]
    pub fn new(gateway: impl Into<String>, service: impl Into<String>, method: impl Into<String>) -> Self {
        Self { gateway: gateway.into(), service: service.into(), method: method.into() }
    }
}

/// Edge from one `file@entry` to another. Resolvers use this, not ad-hoc
/// renderer or scene-bridge code, to build asset dependency graphs.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct AssetEntryDependency {
    pub reference: String,
    pub kind: String,
    pub required: bool,
}

impl Default for AssetEntryDependency {
    fn default() -> Self { Self { reference: String::new(), kind: String::new(), required: true } }
}

impl AssetEntryDependency {
    #[inline]
    pub fn required(reference: impl Into<String>, kind: impl Into<String>) -> Self {
        Self { reference: reference.into(), kind: kind.into(), required: true }
    }

    #[inline]
    pub fn optional(reference: impl Into<String>, kind: impl Into<String>) -> Self {
        Self { reference: reference.into(), kind: kind.into(), required: false }
    }
}

/// One addressable entry inside a dictionary/container file.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct AssetEntryManifest {
    pub name: String,
    pub stable_id: String,
    pub asset_kind: String,
    pub entry_ref: String,
    pub route: AssetGatewayRoute,
    pub dependencies: Vec<AssetEntryDependency>,
    pub metadata: BTreeMap<String, String>,
}

impl Default for AssetEntryManifest {
    fn default() -> Self {
        Self {
            name: String::new(),
            stable_id: String::new(),
            asset_kind: String::new(),
            entry_ref: String::new(),
            route: AssetGatewayRoute::default(),
            dependencies: Vec::new(),
            metadata: BTreeMap::new(),
        }
    }
}

impl AssetEntryManifest {
    #[inline]
    pub fn new(name: impl Into<String>, asset_kind: impl Into<String>, entry_ref: impl Into<String>) -> Self {
        let name = name.into();
        Self { stable_id: stable_id_from_text(&name), name, asset_kind: asset_kind.into(), entry_ref: entry_ref.into(), ..Default::default() }
    }
}

/// Universal `listFiles` result for authored files and package containers.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct AssetFileManifest {
    pub schema: String,
    pub source: String,
    pub file_kind: String,
    pub container: String,
    pub codec: String,
    pub entries: Vec<AssetEntryManifest>,
    pub dependencies: Vec<AssetEntryDependency>,
    pub warnings: Vec<String>,
    pub policy: Vec<String>,
}

impl Default for AssetFileManifest {
    fn default() -> Self {
        Self {
            schema: ASSET_FILE_MANIFEST_SCHEMA.to_owned(),
            source: String::new(),
            file_kind: String::new(),
            container: String::new(),
            codec: String::new(),
            entries: Vec::new(),
            dependencies: Vec::new(),
            warnings: Vec::new(),
            policy: vec![
                "entries are addressed as <logical-path>@entry".to_owned(),
                "logical paths are VFS paths, never physical filesystem paths".to_owned(),
                "domain gateways interpret entries; AssetManager owns bytes and codec dispatch".to_owned(),
            ],
        }
    }
}

#[inline]
pub fn entry_ref(logical_path: &str, entry: &str) -> String {
    let path = logical_path.trim().replace('\\', "/").trim_start_matches('/').to_owned();
    let entry = entry.trim();
    if entry.is_empty() { path } else { format!("{path}@{entry}") }
}

#[inline]
pub fn stable_id_from_text(value: &str) -> String {
    format!("{:016x}", fnv1a64(value.as_bytes()))
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    const OFFSET: u64 = 0xcbf29ce484222325;
    const PRIME: u64 = 0x100000001b3;
    let mut hash = OFFSET;
    for byte in bytes {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(PRIME);
    }
    hash
}
