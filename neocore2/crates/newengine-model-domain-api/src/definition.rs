use serde::{Deserialize, Serialize};

use crate::{
    CLIP_DICTIONARY_ASSET_KIND, CLIP_DICTIONARY_EXTENSION, DEFINITION_ENTRIES_SCHEMA,
    DRAWABLE_DICTIONARY_ASSET_KIND, DRAWABLE_DICTIONARY_EXTENSION, OBJECT_TYPE_DEFINITIONS_ASSET_KIND,
    OBJECT_TYPE_DEFINITIONS_CONTAINER, OBJECT_TYPE_DEFINITIONS_EXTENSION, PHYSICS_DICTIONARY_ASSET_KIND,
    PHYSICS_DICTIONARY_EXTENSION, TEXTURE_DICTIONARY_ASSET_KIND, TEXTURE_DICTIONARY_EXTENSION,
};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DefinitionEntriesRequest {
    pub source: String,
    pub selector: Option<String>,
}

impl Default for DefinitionEntriesRequest {
    fn default() -> Self { Self { source: String::new(), selector: None } }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct DefinitionAssetRef {
    pub name: String,
    pub extension: String,
    pub asset_kind: String,
    pub logical_path_hint: Option<String>,
}

impl Default for DefinitionAssetRef {
    fn default() -> Self {
        Self { name: String::new(), extension: String::new(), asset_kind: String::new(), logical_path_hint: None }
    }
}

impl DefinitionAssetRef {
    pub fn named(name: impl Into<String>, extension: &str, asset_kind: &str) -> Option<Self> {
        let name = name.into();
        let normalized = normalize_definition_asset_name(&name);
        if normalized.is_empty() {
            return None;
        }
        let extension = extension.trim_start_matches('.').to_ascii_lowercase();
        let lower = normalized.to_ascii_lowercase();
        let logical_path_hint = if lower.ends_with(&format!(".{extension}")) {
            lower
        } else {
            format!("{lower}.{extension}")
        };
        Some(Self {
            logical_path_hint: Some(logical_path_hint),
            name: normalized,
            extension,
            asset_kind: asset_kind.to_owned(),
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct DefinitionAssetChain {
    pub definition_type: Option<DefinitionAssetRef>,
    pub drawable_dictionary: Option<DefinitionAssetRef>,
    pub texture_dictionary: Option<DefinitionAssetRef>,
    pub clip_dictionary: Option<DefinitionAssetRef>,
    pub physics_dictionary: Option<DefinitionAssetRef>,
}

impl Default for DefinitionAssetChain {
    fn default() -> Self {
        Self {
            definition_type: None,
            drawable_dictionary: None,
            texture_dictionary: None,
            clip_dictionary: None,
            physics_dictionary: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DefinitionDictionaries {
    pub texture: Option<String>,
    pub drawable: Option<String>,
    pub clip: Option<String>,
    pub physics: Option<String>,
}

impl Default for DefinitionDictionaries {
    fn default() -> Self { Self { texture: None, drawable: None, clip: None, physics: None } }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DefinitionBounds {
    pub bb_min: [f32; 3],
    pub bb_max: [f32; 3],
    pub bs_centre: [f32; 3],
    pub bs_radius: f32,
}

impl Default for DefinitionBounds {
    fn default() -> Self { Self { bb_min: [0.0; 3], bb_max: [0.0; 3], bs_centre: [0.0; 3], bs_radius: 0.0 } }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DefinitionEntry {
    pub entry_kind: String,
    pub name: String,
    pub asset_name: String,
    pub asset_type: String,
    pub lod_dist: f32,
    pub hd_texture_dist: f32,
    pub flags: u32,
    pub special_attribute: u32,
    pub bounds: DefinitionBounds,
    pub dictionaries: DefinitionDictionaries,
    pub asset_chain: DefinitionAssetChain,
}

impl Default for DefinitionEntry {
    fn default() -> Self {
        Self {
            entry_kind: "CBaseArchetypeDef".to_owned(),
            name: String::new(),
            asset_name: String::new(),
            asset_type: String::new(),
            lod_dist: 0.0,
            hd_texture_dist: 0.0,
            flags: 0,
            special_attribute: 0,
            bounds: DefinitionBounds::default(),
            dictionaries: DefinitionDictionaries::default(),
            asset_chain: DefinitionAssetChain::default(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DefinitionEntriesManifest {
    pub schema: String,
    pub codec: String,
    pub source_format: String,
    pub source_encoding: String,
    pub source: String,
    pub name: String,
    pub definition_entries: Vec<DefinitionEntry>,
}

impl Default for DefinitionEntriesManifest {
    fn default() -> Self {
        Self {
            schema: DEFINITION_ENTRIES_SCHEMA.to_owned(),
            codec: "asset.codec.ytyp".to_owned(),
            source_format: OBJECT_TYPE_DEFINITIONS_CONTAINER.to_owned(),
            source_encoding: "utf8.xml".to_owned(),
            source: String::new(),
            name: String::new(),
            definition_entries: Vec::new(),
        }
    }
}

pub fn build_definition_asset_chain(source: &str, entry: &DefinitionEntry) -> DefinitionAssetChain {
    DefinitionAssetChain {
        definition_type: DefinitionAssetRef::named(source, OBJECT_TYPE_DEFINITIONS_EXTENSION, OBJECT_TYPE_DEFINITIONS_ASSET_KIND),
        drawable_dictionary: entry
            .dictionaries
            .drawable
            .as_deref()
            .or_else(|| drawable_name_from_entry(entry))
            .and_then(|name| DefinitionAssetRef::named(name, DRAWABLE_DICTIONARY_EXTENSION, DRAWABLE_DICTIONARY_ASSET_KIND)),
        texture_dictionary: entry
            .dictionaries
            .texture
            .as_deref()
            .and_then(|name| DefinitionAssetRef::named(name, TEXTURE_DICTIONARY_EXTENSION, TEXTURE_DICTIONARY_ASSET_KIND)),
        clip_dictionary: entry
            .dictionaries
            .clip
            .as_deref()
            .and_then(|name| DefinitionAssetRef::named(name, CLIP_DICTIONARY_EXTENSION, CLIP_DICTIONARY_ASSET_KIND)),
        physics_dictionary: entry
            .dictionaries
            .physics
            .as_deref()
            .and_then(|name| DefinitionAssetRef::named(name, PHYSICS_DICTIONARY_EXTENSION, PHYSICS_DICTIONARY_ASSET_KIND)),
    }
}

pub fn refresh_definition_asset_chain(source: &str, entry: &mut DefinitionEntry) {
    entry.asset_chain = build_definition_asset_chain(source, entry);
}

fn drawable_name_from_entry(entry: &DefinitionEntry) -> Option<&str> {
    let asset_type = entry.asset_type.trim();
    if asset_type.eq_ignore_ascii_case("ASSET_TYPE_DRAWABLE")
        || asset_type.eq_ignore_ascii_case("ASSET_TYPE_DRAWABLEDICTIONARY")
    {
        let value = entry.asset_name.trim();
        if !value.is_empty() {
            return Some(value);
        }
        let value = entry.name.trim();
        if !value.is_empty() {
            return Some(value);
        }
    }
    None
}

fn normalize_definition_asset_name(value: &str) -> String {
    let mut s = value.trim().replace('\\', "/");
    while let Some(rest) = s.strip_prefix("./") {
        s = rest.to_owned();
    }
    s = s.trim_start_matches('/').to_owned();
    while s.contains("//") {
        s = s.replace("//", "/");
    }
    s
}
