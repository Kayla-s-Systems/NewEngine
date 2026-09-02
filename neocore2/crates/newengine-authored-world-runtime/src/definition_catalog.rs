use std::{collections::BTreeMap, sync::Arc};

use newengine_definitions_runtime::DefinitionEntryV1;

pub const MAP_DEFINITION_CATALOG_SCHEMA_V1: &str = "newengine.map.definition_catalog.v1";
pub const MAP_DEFINITION_CATALOG_ENCODING_V1: &str = "newengine.map.definition_catalog.indexed.v1";
pub const MAP_DEFINITION_PHYSICAL_DRAWABLE_METADATA_KEY_V1: &str =
    "northstar.map.physical_drawable_ref.v1";
const CATALOG_MAGIC: &[u8; 4] = b"NEDC";
const CATALOG_VERSION: u32 = 1;
const HEADER_LEN: usize = 24;
const INDEX_RECORD_FIXED_LEN: usize = 16;

#[derive(Clone, Copy, Debug)]
struct CatalogEntryRange {
    offset: usize,
    len: usize,
}

#[derive(Clone, Debug)]
pub struct MapDefinitionCatalogV1 {
    map_ref: String,
    entries: BTreeMap<String, CatalogEntryRange>,
    body: Arc<Vec<u8>>,
}

impl MapDefinitionCatalogV1 {
    #[inline]
    pub fn map_ref(&self) -> &str {
        &self.map_ref
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    #[inline]
    pub fn contains(&self, definition_ref: &str) -> bool {
        self.entries
            .contains_key(&normalize_logical_path(definition_ref))
    }

    pub fn decode_entry(&self, definition_ref: &str) -> Result<Option<DefinitionEntryV1>, String> {
        let definition_ref = normalize_logical_path(definition_ref);
        let Some(range) = self.entries.get(&definition_ref).copied() else {
            return Ok(None);
        };
        let payload = checked_slice(&self.body, range.offset, range.len, "definition payload")?;
        let mut entry: DefinitionEntryV1 = serde_json::from_slice(payload).map_err(|error| {
            format!(
                "map definition catalog entry decode failed ref='{}' err='{error}'",
                definition_ref
            )
        })?;
        let identity_ref = normalize_logical_path(&entry.identity.definition_ref);
        if identity_ref != definition_ref {
            return Err(format!(
                "map definition catalog identity mismatch key='{}' identity='{}'",
                definition_ref, identity_ref
            ));
        }
        entry.identity.definition_ref = definition_ref;
        if let Some(physical_ref) = map_definition_physical_drawable_ref(&entry) {
            entry.refs.drawable_refs = vec![physical_ref.clone()];
            entry.model_explanation.drawable_ref = Some(physical_ref);
        }
        Ok(Some(entry))
    }
}

pub fn set_map_definition_physical_drawable_ref(
    entry: &mut DefinitionEntryV1,
    physical_ref: &str,
) -> Result<(), String> {
    let physical_ref = normalize_logical_path(physical_ref);
    let Some((dictionary, selector)) = physical_ref.rsplit_once('@') else {
        return Err(format!(
            "map definition physical drawable ref requires YDD@entry ref='{physical_ref}'"
        ));
    };
    if dictionary.trim().is_empty()
        || !dictionary.to_ascii_lowercase().ends_with(".ydd")
        || selector.trim().is_empty()
    {
        return Err(format!(
            "map definition physical drawable ref is invalid ref='{physical_ref}'"
        ));
    }
    entry.arbitrary_metadata.insert(
        MAP_DEFINITION_PHYSICAL_DRAWABLE_METADATA_KEY_V1.to_owned(),
        serde_json::Value::String(physical_ref),
    );
    Ok(())
}

pub fn map_definition_physical_drawable_ref(entry: &DefinitionEntryV1) -> Option<String> {
    entry
        .arbitrary_metadata
        .get(MAP_DEFINITION_PHYSICAL_DRAWABLE_METADATA_KEY_V1)
        .and_then(serde_json::Value::as_str)
        .map(normalize_logical_path)
        .filter(|value| !value.is_empty())
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum MapDefinitionCatalogLoad {
    Missing {
        path: String,
    },
    Loaded {
        path: String,
        entries: usize,
        bytes: usize,
    },
}

fn normalize_logical_path(value: &str) -> String {
    value
        .trim()
        .replace('\\', "/")
        .trim_start_matches("./")
        .trim_start_matches('/')
        .to_owned()
}

pub fn map_definition_catalog_path(map_ref: &str) -> Result<String, String> {
    let logical = normalize_logical_path(map_ref.split('@').next().unwrap_or(map_ref));
    let lower = logical.to_ascii_lowercase();
    if !lower.ends_with(".ymap") {
        return Err(format!(
            "map definition catalog path requires .ymap logical ref map_ref='{map_ref}'"
        ));
    }
    let stem = &logical[..logical.len() - ".ymap".len()];
    Ok(format!("{stem}.definition_catalog"))
}

pub fn encode_map_definition_catalog(
    map_ref: &str,
    entries: BTreeMap<String, DefinitionEntryV1>,
) -> Result<Vec<u8>, String> {
    let map_ref = normalize_logical_path(map_ref.split('@').next().unwrap_or(map_ref));
    if !map_ref.to_ascii_lowercase().ends_with(".ymap") {
        return Err(format!(
            "map definition catalog owner must be a .ymap logical ref map_ref='{map_ref}'"
        ));
    }
    if entries.is_empty() {
        return Err(format!(
            "map definition catalog cannot encode an empty entry set map='{map_ref}'"
        ));
    }

    let mut encoded_entries = Vec::with_capacity(entries.len());
    let mut index_len = 0usize;
    for (key, entry) in entries {
        let key = normalize_logical_path(&key);
        let identity_ref = normalize_logical_path(&entry.identity.definition_ref);
        if key.is_empty() || identity_ref != key {
            return Err(format!(
                "map definition catalog encode identity mismatch key='{}' identity='{}'",
                key, identity_ref
            ));
        }
        let payload = serde_json::to_vec(&entry).map_err(|error| {
            format!("map definition catalog entry encode failed ref='{key}' err='{error}'")
        })?;
        index_len = index_len
            .checked_add(INDEX_RECORD_FIXED_LEN)
            .and_then(|value| value.checked_add(key.len()))
            .ok_or("map definition catalog index size overflow")?;
        encoded_entries.push((key, payload));
    }

    let payload_floor = HEADER_LEN
        .checked_add(map_ref.len())
        .and_then(|value| value.checked_add(index_len))
        .ok_or("map definition catalog payload floor overflow")?;
    let mut total_len = payload_floor;
    for (_, payload) in &encoded_entries {
        total_len = total_len
            .checked_add(payload.len())
            .ok_or("map definition catalog total size overflow")?;
    }

    let entry_count = u32::try_from(encoded_entries.len())
        .map_err(|_| "map definition catalog entry count exceeds u32".to_owned())?;
    let map_ref_len = u32::try_from(map_ref.len())
        .map_err(|_| "map definition catalog map_ref length exceeds u32".to_owned())?;
    let payload_floor_u64 = u64::try_from(payload_floor)
        .map_err(|_| "map definition catalog payload floor exceeds u64".to_owned())?;

    let mut out = Vec::with_capacity(total_len);
    out.extend_from_slice(CATALOG_MAGIC);
    out.extend_from_slice(&CATALOG_VERSION.to_le_bytes());
    out.extend_from_slice(&map_ref_len.to_le_bytes());
    out.extend_from_slice(&entry_count.to_le_bytes());
    out.extend_from_slice(&payload_floor_u64.to_le_bytes());
    out.extend_from_slice(map_ref.as_bytes());

    let mut payload_cursor = payload_floor;
    for (key, payload) in &encoded_entries {
        let key_len = u32::try_from(key.len())
            .map_err(|_| format!("map definition catalog key too large ref='{key}'"))?;
        let payload_len = u32::try_from(payload.len())
            .map_err(|_| format!("map definition catalog payload too large ref='{key}'"))?;
        let payload_offset = u64::try_from(payload_cursor)
            .map_err(|_| "map definition catalog payload offset exceeds u64".to_owned())?;
        out.extend_from_slice(&key_len.to_le_bytes());
        out.extend_from_slice(&payload_len.to_le_bytes());
        out.extend_from_slice(&payload_offset.to_le_bytes());
        out.extend_from_slice(key.as_bytes());
        payload_cursor = payload_cursor
            .checked_add(payload.len())
            .ok_or("map definition catalog payload cursor overflow")?;
    }
    debug_assert_eq!(out.len(), payload_floor);
    for (_, payload) in encoded_entries {
        out.extend_from_slice(&payload);
    }
    debug_assert_eq!(out.len(), total_len);
    Ok(out)
}

pub fn decode_map_definition_catalog(
    expected_map_ref: &str,
    body: &[u8],
) -> Result<MapDefinitionCatalogV1, String> {
    decode_map_definition_catalog_owned(expected_map_ref, body.to_vec())
}

pub fn decode_map_definition_catalog_owned(
    expected_map_ref: &str,
    body: Vec<u8>,
) -> Result<MapDefinitionCatalogV1, String> {
    if body.len() < HEADER_LEN {
        return Err(format!(
            "map definition catalog body too small bytes={} expected>={HEADER_LEN}",
            body.len()
        ));
    }
    if &body[..4] != CATALOG_MAGIC {
        return Err("map definition catalog magic mismatch".to_owned());
    }
    let version = read_u32(&body, 4)?;
    if version != CATALOG_VERSION {
        return Err(format!(
            "unsupported map definition catalog version={version} expected={CATALOG_VERSION}"
        ));
    }
    let map_ref_len = read_u32(&body, 8)? as usize;
    let entry_count = read_u32(&body, 12)? as usize;
    if entry_count == 0 {
        return Err("map definition catalog contains no entries".to_owned());
    }
    let payload_floor = usize::try_from(read_u64(&body, 16)?)
        .map_err(|_| "map definition catalog payload floor exceeds usize".to_owned())?;
    if payload_floor > body.len() {
        return Err("map definition catalog payload floor outside body".to_owned());
    }
    let map_ref_bytes = checked_slice(&body, HEADER_LEN, map_ref_len, "map_ref")?;
    let map_ref = std::str::from_utf8(map_ref_bytes)
        .map_err(|error| format!("map definition catalog map_ref is not UTF-8: {error}"))?;
    let map_ref = normalize_logical_path(map_ref);
    let expected_map = normalize_logical_path(
        expected_map_ref
            .split('@')
            .next()
            .unwrap_or(expected_map_ref),
    );
    if map_ref != expected_map {
        return Err(format!(
            "map definition catalog owner mismatch expected='{}' actual='{}'",
            expected_map, map_ref
        ));
    }

    let mut cursor = HEADER_LEN + map_ref_len;
    let mut entries = BTreeMap::new();
    for _ in 0..entry_count {
        if cursor + INDEX_RECORD_FIXED_LEN > payload_floor {
            return Err("map definition catalog index record outside index region".to_owned());
        }
        let key_len = read_u32(&body, cursor)? as usize;
        let payload_len = read_u32(&body, cursor + 4)? as usize;
        let payload_offset = usize::try_from(read_u64(&body, cursor + 8)?)
            .map_err(|_| "map definition catalog payload offset exceeds usize".to_owned())?;
        cursor += INDEX_RECORD_FIXED_LEN;
        let key_bytes = checked_slice(&body, cursor, key_len, "definition key")?;
        if cursor + key_len > payload_floor {
            return Err("map definition catalog key outside index region".to_owned());
        }
        let key = std::str::from_utf8(key_bytes)
            .map_err(|error| format!("map definition catalog key is not UTF-8: {error}"))?;
        let key = normalize_logical_path(key);
        cursor += key_len;
        if payload_offset < payload_floor {
            return Err(format!(
                "map definition catalog payload precedes payload region ref='{key}'"
            ));
        }
        checked_slice(&body, payload_offset, payload_len, "definition payload")?;
        if entries
            .insert(
                key.clone(),
                CatalogEntryRange {
                    offset: payload_offset,
                    len: payload_len,
                },
            )
            .is_some()
        {
            return Err(format!(
                "map definition catalog contains duplicate definition_ref='{key}'"
            ));
        }
    }
    if cursor != payload_floor {
        return Err(format!(
            "map definition catalog index has trailing bytes trailing={}",
            payload_floor - cursor
        ));
    }

    Ok(MapDefinitionCatalogV1 {
        map_ref,
        entries,
        body: Arc::new(body),
    })
}

pub(crate) fn load_map_definition_catalog(
    logical_map_ref: &str,
) -> Result<(MapDefinitionCatalogLoad, Option<MapDefinitionCatalogV1>), String> {
    let path = map_definition_catalog_path(logical_map_ref)?;
    let assets =
        newengine_assets_api::AssetServiceClient::new(newengine_plugin_host::default_host_api());
    let body = match assets.raw_bytes_v1(&path) {
        Ok(body) => body,
        Err(_) => {
            return Ok((MapDefinitionCatalogLoad::Missing { path }, None));
        }
    };
    let bytes = body.len();
    let catalog = decode_map_definition_catalog_owned(logical_map_ref, body)
        .map_err(|error| format!("map definition catalog invalid path='{path}' err='{error}'"))?;
    let entries = catalog.len();
    Ok((
        MapDefinitionCatalogLoad::Loaded {
            path,
            entries,
            bytes,
        },
        Some(catalog),
    ))
}

#[inline]
fn checked_slice<'a>(
    bytes: &'a [u8],
    offset: usize,
    len: usize,
    label: &str,
) -> Result<&'a [u8], String> {
    let end = offset
        .checked_add(len)
        .ok_or_else(|| format!("map definition catalog {label} range overflow"))?;
    bytes.get(offset..end).ok_or_else(|| {
        format!("map definition catalog {label} outside body offset={offset} len={len}")
    })
}

#[inline]
fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, String> {
    let value = checked_slice(bytes, offset, 4, "u32")?;
    Ok(u32::from_le_bytes(value.try_into().expect("u32 slice")))
}

#[inline]
fn read_u64(bytes: &[u8], offset: usize) -> Result<u64, String> {
    let value = checked_slice(bytes, offset, 8, "u64")?;
    Ok(u64::from_le_bytes(value.try_into().expect("u64 slice")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn definition(reference: &str) -> DefinitionEntryV1 {
        let mut entry = DefinitionEntryV1::default();
        let (source, name) = reference.rsplit_once('@').unwrap();
        entry.identity.source = source.to_owned();
        entry.identity.name = name.to_owned();
        entry.identity.definition_ref = reference.to_owned();
        entry.refs.drawable_refs = vec![format!("models/world.ydd@{name}")];
        entry
    }

    #[test]
    fn catalog_path_is_sibling_of_map() {
        assert_eq!(
            map_definition_catalog_path(r"maps\seattle_forward_base.ymap@map").unwrap(),
            "maps/seattle_forward_base.definition_catalog"
        );
    }

    #[test]
    fn indexed_catalog_round_trips_selected_entry() {
        let a = "definitions/maps/demo/tree.ytyp@tree";
        let b = "definitions/maps/demo/wall.ytyp@wall";
        let encoded = encode_map_definition_catalog(
            "maps/demo.ymap",
            BTreeMap::from([(a.to_owned(), definition(a)), (b.to_owned(), definition(b))]),
        )
        .unwrap();
        let decoded = decode_map_definition_catalog("maps/demo.ymap@map", &encoded).unwrap();
        assert_eq!(decoded.len(), 2);
        assert!(decoded.contains(a));
        assert_eq!(
            decoded.decode_entry(b).unwrap().unwrap().refs.drawable_refs,
            vec!["models/world.ydd@wall"]
        );
    }

    #[test]
    fn catalog_decode_rejects_stale_owner() {
        let reference = "definitions/maps/demo/tree.ytyp@tree";
        let encoded = encode_map_definition_catalog(
            "maps/other.ymap",
            BTreeMap::from([(reference.to_owned(), definition(reference))]),
        )
        .unwrap();
        let error = decode_map_definition_catalog("maps/demo.ymap", &encoded).unwrap_err();
        assert!(error.contains("owner mismatch"));
    }

    #[test]
    fn selected_entry_identity_is_validated_on_demand() {
        let reference = "definitions/maps/demo/tree.ytyp@tree";
        let mut wrong = definition(reference);
        wrong.identity.definition_ref = "definitions/maps/demo/other.ytyp@other".to_owned();
        let error = encode_map_definition_catalog(
            "maps/demo.ymap",
            BTreeMap::from([(reference.to_owned(), wrong)]),
        )
        .unwrap_err();
        assert!(error.contains("identity mismatch"));
    }

    #[test]
    fn physical_drawable_override_is_applied_only_when_catalog_entry_is_decoded() {
        let reference = "definitions/maps/demo/tree.ytyp@tree";
        let mut entry = definition(reference);
        set_map_definition_physical_drawable_ref(
            &mut entry,
            "models/maps/demo_pages/cell_0_0_00.ydd@tree",
        )
        .unwrap();
        assert_eq!(
            entry.refs.drawable_refs,
            vec!["models/world.ydd@tree"],
            "semantic DTO remains authoritative before catalog resolution"
        );
        let encoded = encode_map_definition_catalog(
            "maps/demo.ymap",
            BTreeMap::from([(reference.to_owned(), entry)]),
        )
        .unwrap();
        let catalog = decode_map_definition_catalog("maps/demo.ymap", &encoded).unwrap();
        let decoded = catalog.decode_entry(reference).unwrap().unwrap();
        assert_eq!(
            decoded.refs.drawable_refs,
            vec!["models/maps/demo_pages/cell_0_0_00.ydd@tree"]
        );
        assert_eq!(
            decoded.model_explanation.drawable_ref.as_deref(),
            Some("models/maps/demo_pages/cell_0_0_00.ydd@tree")
        );
        assert_eq!(
            map_definition_physical_drawable_ref(&decoded).as_deref(),
            Some("models/maps/demo_pages/cell_0_0_00.ydd@tree")
        );
    }
}
