use serde::{Deserialize, Serialize};

use crate::{
    DRAWABLE_DICTIONARY_ASSET_KIND, DRAWABLE_DICTIONARY_CONTAINER, DRAWABLE_DICTIONARY_MANIFEST_SCHEMA,
};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DrawableDictionaryRequest {
    pub source: String,
    pub selector: Option<String>,
}

impl Default for DrawableDictionaryRequest {
    fn default() -> Self { Self { source: String::new(), selector: None } }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DrawableDictionaryEntry {
    pub name: String,
    pub name_hash: u64,
    pub mesh_count: u32,
    pub material_slots: Vec<String>,
    pub bounds_min: [f32; 3],
    pub bounds_max: [f32; 3],
}

impl Default for DrawableDictionaryEntry {
    fn default() -> Self {
        Self {
            name: String::new(),
            name_hash: 0,
            mesh_count: 0,
            material_slots: Vec::new(),
            bounds_min: [0.0; 3],
            bounds_max: [0.0; 3],
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DrawableDictionaryManifest {
    pub schema: String,
    pub source: String,
    pub asset_kind: String,
    pub container: String,
    pub texture_dictionary: Option<String>,
    pub entries: Vec<DrawableDictionaryEntry>,
}

impl Default for DrawableDictionaryManifest {
    fn default() -> Self {
        Self {
            schema: DRAWABLE_DICTIONARY_MANIFEST_SCHEMA.to_owned(),
            source: String::new(),
            asset_kind: DRAWABLE_DICTIONARY_ASSET_KIND.to_owned(),
            container: DRAWABLE_DICTIONARY_CONTAINER.to_owned(),
            texture_dictionary: None,
            entries: Vec::new(),
        }
    }
}
