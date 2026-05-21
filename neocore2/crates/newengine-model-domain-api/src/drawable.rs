use serde::{Deserialize, Serialize};

use crate::{
    DRAWABLE_DICTIONARY_ASSET_KIND, DRAWABLE_DICTIONARY_CONTAINER, DRAWABLE_DICTIONARY_MANIFEST_SCHEMA,
};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DrawableDictionaryRequest { pub source: String, pub selector: Option<String> }
impl Default for DrawableDictionaryRequest { fn default() -> Self { Self { source: String::new(), selector: None } } }

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct DrawableMaterialSlotRef { pub slot: String, pub material: String }
impl Default for DrawableMaterialSlotRef { fn default() -> Self { Self { slot: String::new(), material: String::new() } } }

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DrawableDictionaryEntry {
    pub name: String,
    pub name_hash: u64,
    pub mesh_count: u32,
    /// Declarative material slot references. Drawable dictionaries never store
    /// renderer-owned material state; they point to material descriptors such as
    /// `player/abigail/materials/abigail_skin.nemat@head`.
    pub material_slots: Vec<DrawableMaterialSlotRef>,
    pub skeleton_refs: Vec<String>,
    pub lods: Vec<String>,
    pub collision_refs: Vec<String>,
    pub dependency_refs: Vec<String>,
    pub bounds_min: [f32; 3],
    pub bounds_max: [f32; 3],
}
impl Default for DrawableDictionaryEntry {
    fn default() -> Self { Self { name: String::new(), name_hash: 0, mesh_count: 0, material_slots: Vec::new(), skeleton_refs: Vec::new(), lods: Vec::new(), collision_refs: Vec::new(), dependency_refs: Vec::new(), bounds_min: [0.0; 3], bounds_max: [0.0; 3] } }
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
    pub warnings: Vec<String>,
}
impl Default for DrawableDictionaryManifest {
    fn default() -> Self { Self { schema: DRAWABLE_DICTIONARY_MANIFEST_SCHEMA.to_owned(), source: String::new(), asset_kind: DRAWABLE_DICTIONARY_ASSET_KIND.to_owned(), container: DRAWABLE_DICTIONARY_CONTAINER.to_owned(), texture_dictionary: None, entries: Vec::new(), warnings: Vec::new() } }
}
