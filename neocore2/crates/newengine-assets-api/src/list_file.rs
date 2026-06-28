#![forbid(unsafe_op_in_unsafe_fn)]

use std::{collections::BTreeMap, ops::Range};

/// Canonical manifest schema returned by dictionary/container codecs for the
/// data-driven asset world.
///
/// `listFiles` answers one generic question for every format: which addressable
/// entries does this logical file expose, and through which gateway/method should
/// those entries be resolved?
pub const ASSET_FILE_MANIFEST_SCHEMA: &str = "newengine.asset.list_files.v1";
pub const ASSET_LIST_FILE_MANIFEST_OUTPUT: &str = "asset.list_file_manifest_v1";
pub const ASSET_LIST_FILE_HEADER_OUTPUT: &str = "asset.list_file_header_v1";
pub const ASSET_LIST_FILE_BODY_OUTPUT: &str = "asset.list_file_body_v1";

/// Declarative route from an asset entry to the gateway that owns its semantic
/// interpretation. AssetManager still owns VFS bytes/codec dispatch; domain
/// gateways own meaning. This is semantic metadata, not a provider-service id.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct AssetGatewayRoute {
    pub gateway: String,
    pub method: String,
    pub semantic_owner: String,
}

impl Default for AssetGatewayRoute {
    fn default() -> Self {
        Self {
            gateway: "engine.assets".to_owned(),
            method: "asset.decode_v1".to_owned(),
            semantic_owner: "asset".to_owned(),
        }
    }
}

impl AssetGatewayRoute {
    #[inline]
    pub fn new(
        gateway: impl Into<String>,
        method: impl Into<String>,
        semantic_owner: impl Into<String>,
    ) -> Self {
        Self {
            gateway: gateway.into(),
            method: method.into(),
            semantic_owner: semantic_owner.into(),
        }
    }
}

/// Edge from one `file@entry` to another. Resolvers use this, not ad-hoc
/// renderer or scene-bridge code, to build asset dependency graphs.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct AssetEntryDependency {
    pub reference: String,
    /// Compatibility projection for older manifest consumers. New graph code should prefer `role`.
    pub kind: String,
    /// Semantic edge role, e.g. `material_slot/head` or `texture/base_color`.
    pub role: String,
    /// Owning semantic domain for the referenced entry, e.g. `engine.materials`.
    pub domain: String,
    pub required: bool,
}

impl Default for AssetEntryDependency {
    fn default() -> Self {
        Self {
            reference: String::new(),
            kind: String::new(),
            role: String::new(),
            domain: String::new(),
            required: true,
        }
    }
}

impl AssetEntryDependency {
    #[inline]
    pub fn required(reference: impl Into<String>, kind: impl Into<String>) -> Self {
        let kind = kind.into();
        Self {
            reference: reference.into(),
            role: kind.clone(),
            kind,
            domain: String::new(),
            required: true,
        }
    }

    #[inline]
    pub fn optional(reference: impl Into<String>, kind: impl Into<String>) -> Self {
        let kind = kind.into();
        Self {
            reference: reference.into(),
            role: kind.clone(),
            kind,
            domain: String::new(),
            required: false,
        }
    }

    #[inline]
    pub fn domain(mut self, domain: impl Into<String>) -> Self {
        self.domain = domain.into();
        self
    }

    #[inline]
    pub fn role(mut self, role: impl Into<String>) -> Self {
        let role = role.into();
        self.kind = role.clone();
        self.role = role;
        self
    }
}

/// Common dependency record used inside NEF8/ListFile domain bodies.
///
/// This is the binary-domain counterpart of `AssetEntryDependency`: codecs and
/// domain handlers can preserve exact roles without forcing AssetManager to know
/// material/model/texture semantics.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct AssetDependencyRecordV1 {
    pub reference: String,
    pub role: String,
    pub required: bool,
    pub domain: String,
}

impl Default for AssetDependencyRecordV1 {
    fn default() -> Self {
        Self {
            reference: String::new(),
            role: String::new(),
            required: true,
            domain: String::new(),
        }
    }
}

impl AssetDependencyRecordV1 {
    #[inline]
    pub fn new(
        reference: impl Into<String>,
        role: impl Into<String>,
        domain: impl Into<String>,
        required: bool,
    ) -> Self {
        Self {
            reference: reference.into(),
            role: role.into(),
            domain: domain.into(),
            required,
        }
    }
}

/// Common addressable entry record for every NEF8/ListFile asset dictionary.
///
/// Domain bodies may keep their own compact tables, but they should project this
/// shape for asset catalog UI projections, AssetGraphResolver and conformance tests. Payload
/// ranges are relative to the inflated NEF8 body, never to a filesystem path.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct ListFileEntryRecordV1 {
    pub name: String,
    pub stable_hash: u64,
    pub entry_kind: String,
    pub schema: String,
    pub payload_offset: u64,
    pub payload_len: u64,
    pub payload_hash: [u8; 32],
    pub dependencies_range: Range<u32>,
    pub metadata_range: Range<u32>,
    pub flags: u32,
}

impl Default for ListFileEntryRecordV1 {
    fn default() -> Self {
        Self {
            name: String::new(),
            stable_hash: 0,
            entry_kind: String::new(),
            schema: String::new(),
            payload_offset: 0,
            payload_len: 0,
            payload_hash: [0; 32],
            dependencies_range: 0..0,
            metadata_range: 0..0,
            flags: 0,
        }
    }
}

impl ListFileEntryRecordV1 {
    #[inline]
    pub fn new(
        name: impl Into<String>,
        entry_kind: impl Into<String>,
        schema: impl Into<String>,
    ) -> Self {
        let name = name.into();
        Self {
            stable_hash: stable_hash_from_text(&name),
            name,
            entry_kind: entry_kind.into(),
            schema: schema.into(),
            ..Default::default()
        }
    }
}

/// Common metadata namespace blob. Unknown namespaces must be preserved by tools
/// and ignored by runtime domains that do not own them.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct ListFileMetadataNamespaceV1 {
    pub namespace: String,
    pub schema: String,
    pub payload_offset: u64,
    pub payload_len: u64,
}

impl Default for ListFileMetadataNamespaceV1 {
    fn default() -> Self {
        Self {
            namespace: String::new(),
            schema: String::new(),
            payload_offset: 0,
            payload_len: 0,
        }
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
    pub fn new(
        name: impl Into<String>,
        asset_kind: impl Into<String>,
        entry_ref: impl Into<String>,
    ) -> Self {
        let name = name.into();
        Self {
            stable_id: stable_id_from_text(&name),
            name,
            asset_kind: asset_kind.into(),
            entry_ref: entry_ref.into(),
            ..Default::default()
        }
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
                "domain gateways interpret entries; AssetManager owns bytes and codec dispatch"
                    .to_owned(),
            ],
        }
    }
}

#[inline]
pub fn entry_ref(logical_path: &str, entry: &str) -> String {
    let path = logical_path
        .trim()
        .replace('\\', "/")
        .trim_start_matches('/')
        .to_owned();
    let entry = entry.trim();
    if entry.is_empty() {
        path
    } else {
        format!("{path}@{entry}")
    }
}

#[inline]
pub fn stable_id_from_text(value: &str) -> String {
    format!("{:016x}", stable_hash_from_text(value))
}

#[inline]
pub fn stable_hash_from_text(value: &str) -> u64 {
    fnv1a64(value.as_bytes())
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

/// Binary ListFile envelope magic used by every authored asset dictionary file.
///
/// File extensions stay domain-facing (`.ytyp`, `.ytd`, `.ydd`, `.nemat`, ...),
/// but the top-level binary envelope is always NEF8. Concrete content identity is
/// declared by `content_kind`, not by per-format magic bytes.
pub const LIST_FILE_MAGIC_NEF8: [u8; 4] = *b"NEF8";
pub const LIST_FILE_VERSION_V1: u16 = 1;
pub const LIST_FILE_HEADER_LEN_V1: usize = 128;
pub const LIST_FILE_FLAG_BODY_DEFLATE: u16 = 0x0001;
pub const LIST_FILE_COMPRESSION_DEFLATE: u16 = 1;

pub const LIST_FILE_CONTENT_KIND_UNKNOWN: u32 = 0;
pub const LIST_FILE_CONTENT_KIND_YTD: u32 = 1;
pub const LIST_FILE_CONTENT_KIND_YDD: u32 = 2;
pub const LIST_FILE_CONTENT_KIND_YTYP: u32 = 3;
pub const LIST_FILE_CONTENT_KIND_NEMAT: u32 = 4;
pub const LIST_FILE_CONTENT_KIND_YMAP: u32 = 5;
pub const LIST_FILE_CONTENT_KIND_YDR: u32 = 6;
pub const LIST_FILE_CONTENT_KIND_YFT: u32 = 7;
/// North Star Font Dictionary: resident NEF8/ListFile font dictionary used by engine.ui.text.
/// NOTE: currently shares kind 8 with legacy YBN until the historical YBN slot is migrated.
pub const LIST_FILE_CONTENT_KIND_NEFTD: u32 = 8;
pub const LIST_FILE_CONTENT_KIND_YBN: u32 = 8;
pub const LIST_FILE_CONTENT_KIND_YMF: u32 = 9;
pub const LIST_FILE_CONTENT_KIND_YMT: u32 = 10;
pub const LIST_FILE_CONTENT_KIND_YCD: u32 = 11;
pub const LIST_FILE_CONTENT_KIND_YED: u32 = 12;
pub const LIST_FILE_CONTENT_KIND_YFD: u32 = 13;
pub const LIST_FILE_CONTENT_KIND_YLD: u32 = 14;
pub const LIST_FILE_CONTENT_KIND_YPDB: u32 = 15;
pub const LIST_FILE_CONTENT_KIND_YVR: u32 = 16;
pub const LIST_FILE_CONTENT_KIND_YWR: u32 = 17;
pub const LIST_FILE_CONTENT_KIND_YSC: u32 = 18;
pub const LIST_FILE_CONTENT_KIND_YBD: u32 = 19;
pub const LIST_FILE_CONTENT_KIND_YTF: u32 = 20;
/// NewEngine UI dictionary: surfaces/layouts/themes/components/bindings in XMLcentral payload.
pub const LIST_FILE_CONTENT_KIND_NEUI: u32 = 32;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ListFileHeaderV1 {
    pub version: u16,
    pub header_len: u16,
    pub content_kind: u32,
    pub content_schema_version: u16,
    pub flags: u16,
    pub compression: u16,
    pub entry_count: u32,
    pub header_metadata_offset: u64,
    pub header_metadata_len: u64,
    pub body_offset: u64,
    pub body_len: u64,
    pub body_uncompressed_len: u64,
    /// BLAKE3 hash of the inflated NEF8 body bytes.
    pub body_raw_hash: [u8; 32],
    pub import_settings_hash: u64,
    pub stable_file_id: u64,
}

impl ListFileHeaderV1 {
    #[inline]
    pub fn content_kind_label(&self) -> &'static str {
        list_file_content_kind_label(self.content_kind)
    }

    #[inline]
    pub fn is_deflate_body(&self) -> bool {
        (self.flags & LIST_FILE_FLAG_BODY_DEFLATE) != 0
            && self.compression == LIST_FILE_COMPRESSION_DEFLATE
    }
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct ListFileHeaderMetadata {
    pub schema: String,
    pub logical_path: String,
    pub content_kind: String,
    pub authored_by: String,
    pub source: String,
    pub build_profile: String,
    pub asset_graph_hash: String,
    pub import_settings_hash: String,
    pub entries: Vec<AssetEntryManifest>,
    pub dependencies: Vec<AssetEntryDependency>,
    pub warnings: Vec<String>,
    pub policy: Vec<String>,
}

impl Default for ListFileHeaderMetadata {
    fn default() -> Self {
        Self {
            schema: "newengine.asset.list_file.header_metadata.v1".to_owned(),
            logical_path: String::new(),
            content_kind: String::new(),
            authored_by: String::new(),
            source: String::new(),
            build_profile: String::new(),
            asset_graph_hash: String::new(),
            import_settings_hash: String::new(),
            entries: Vec::new(),
            dependencies: Vec::new(),
            warnings: Vec::new(),
            policy: vec![
                "NEF8 is the only top-level magic for authored asset dictionary files".to_owned(),
                "content_kind in the header selects the domain payload".to_owned(),
                "body is deflate-compressed and decoded by the content domain handler".to_owned(),
            ],
        }
    }
}

#[inline]
pub const fn list_file_content_kind_label(kind: u32) -> &'static str {
    match kind {
        LIST_FILE_CONTENT_KIND_UNKNOWN => "unknown",
        LIST_FILE_CONTENT_KIND_YTD => "ytd_texture_dictionary",
        LIST_FILE_CONTENT_KIND_YDD => "ydd_drawable_dictionary",
        LIST_FILE_CONTENT_KIND_YTYP => "ytyp_archetype_dictionary",
        LIST_FILE_CONTENT_KIND_NEMAT => "nemat_material_library",
        LIST_FILE_CONTENT_KIND_YMAP => "ymap_map_data",
        LIST_FILE_CONTENT_KIND_YDR => "ydr_drawable",
        LIST_FILE_CONTENT_KIND_YFT => "yft_fragment",
        LIST_FILE_CONTENT_KIND_NEFTD => "neftd_or_ybn_dictionary",
        LIST_FILE_CONTENT_KIND_YMF => "ymf_manifest",
        LIST_FILE_CONTENT_KIND_YMT => "ymt_metadata",
        LIST_FILE_CONTENT_KIND_YCD => "ycd_clip_dictionary",
        LIST_FILE_CONTENT_KIND_YED => "yed_expression_dictionary",
        LIST_FILE_CONTENT_KIND_YFD => "yfd_frame_filter_dictionary",
        LIST_FILE_CONTENT_KIND_YLD => "yld_cloth_dictionary",
        LIST_FILE_CONTENT_KIND_YPDB => "ypdb_pose_database",
        LIST_FILE_CONTENT_KIND_YVR => "yvr_vehicle_record",
        LIST_FILE_CONTENT_KIND_YWR => "ywr_waypoint_record",
        LIST_FILE_CONTENT_KIND_YSC => "ysc_script_module",
        LIST_FILE_CONTENT_KIND_YBD => "ybd_bounds_dictionary",
        LIST_FILE_CONTENT_KIND_YTF => "ytf_unknown_y_file",
        LIST_FILE_CONTENT_KIND_NEUI => "neui_ui_dictionary",
        _ => "provider_declared",
    }
}

/// Descriptor-local NEF8/ListFile format metadata.
///
/// This struct is retained for format crates/tools that want a compact local
/// declaration, but the asset API no longer owns a global `LIST_FILE_FORMAT_SPECS`
/// table. Concrete formats must publish descriptors from their own crates or
/// providers and register them with `engine.assets.types`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ListFileFormatSpec {
    pub extension: &'static str,
    pub content_kind: u32,
    pub asset_kind: &'static str,
    pub purpose: &'static str,
    pub semantic_gateway: &'static str,
    pub handler_service: &'static str,
    pub selector_syntax: &'static str,
}

pub fn parse_list_file_header_v1(bytes: &[u8]) -> Result<ListFileHeaderV1, String> {
    if bytes.len() < LIST_FILE_HEADER_LEN_V1 {
        return Err(format!(
            "NEF8 ListFile header too small: bytes={} expected>={}",
            bytes.len(),
            LIST_FILE_HEADER_LEN_V1
        ));
    }
    if bytes.get(0..4) != Some(&LIST_FILE_MAGIC_NEF8[..]) {
        return Err("NEF8 ListFile magic mismatch".to_owned());
    }
    let version = read_u16(bytes, 4)?;
    if version != LIST_FILE_VERSION_V1 {
        return Err(format!("unsupported NEF8 ListFile version {version}"));
    }
    let header_len = read_u16(bytes, 6)?;
    if (header_len as usize) > bytes.len() || (header_len as usize) < LIST_FILE_HEADER_LEN_V1 {
        return Err(format!("invalid NEF8 ListFile header_len={header_len}"));
    }

    let content_kind = read_u16(bytes, 8)? as u32;
    if content_kind == LIST_FILE_CONTENT_KIND_UNKNOWN {
        return Err("NEF8 ListFile content_kind unknown/invalid".to_owned());
    }

    let flags = read_u16(bytes, 10)?;
    let compression = read_u16(bytes, 12)?;
    if compression != LIST_FILE_COMPRESSION_DEFLATE {
        return Err(format!(
            "unsupported NEF8 ListFile body compression {compression}"
        ));
    }
    if (flags & LIST_FILE_FLAG_BODY_DEFLATE) == 0 {
        return Err(format!(
            "NEF8 ListFile missing deflate body flag flags=0x{flags:04x}"
        ));
    }

    let entry_count_u64 = read_u64(bytes, 40)?;
    let entry_count = u32::try_from(entry_count_u64)
        .map_err(|_| format!("NEF8 ListFile entry_count too large: {entry_count_u64}"))?;

    // Canonical NEF8HeaderV1 layout:
    // 0x08 u16 content_kind, 0x0A u16 flags, 0x0C u16 compression,
    // 0x10.. body range, 0x28 entry_count, 0x30 metadata range,
    // 0x40 body_raw_hash[32] (BLAKE3), 0x60 file uuid/stable id,
    // 0x70 schema version.
    Ok(ListFileHeaderV1 {
        version,
        header_len,
        content_kind,
        content_schema_version: read_u64(bytes, 112).unwrap_or(1) as u16,
        flags,
        compression,
        entry_count,
        body_offset: read_u64(bytes, 16)?,
        body_len: read_u64(bytes, 24)?,
        body_uncompressed_len: read_u64(bytes, 32)?,
        header_metadata_offset: read_u64(bytes, 48)?,
        header_metadata_len: read_u64(bytes, 56)?,
        body_raw_hash: read_hash32(bytes, 64)?,
        import_settings_hash: 0,
        stable_file_id: read_u64(bytes, 96).unwrap_or(0),
    })
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16, String> {
    let slice = bytes
        .get(offset..offset + 2)
        .ok_or_else(|| format!("NEF8 header truncated at u16 offset {offset}"))?;
    Ok(u16::from_le_bytes([slice[0], slice[1]]))
}

fn read_u64(bytes: &[u8], offset: usize) -> Result<u64, String> {
    let slice = bytes
        .get(offset..offset + 8)
        .ok_or_else(|| format!("NEF8 header truncated at u64 offset {offset}"))?;
    Ok(u64::from_le_bytes([
        slice[0], slice[1], slice[2], slice[3], slice[4], slice[5], slice[6], slice[7],
    ]))
}

fn read_hash32(bytes: &[u8], offset: usize) -> Result<[u8; 32], String> {
    let slice = bytes
        .get(offset..offset + 32)
        .ok_or_else(|| format!("NEF8 header truncated at hash32 offset {offset}"))?;
    let mut out = [0u8; 32];
    out.copy_from_slice(slice);
    Ok(out)
}
