use std::collections::BTreeMap;

use newengine_assets_api::list_file::{
    AssetDependencyRecord, ListFileEntryRecord, ListFileMetadataNamespace,
};

use crate::texture_refs::validate_material_texture_reference;

/// Authored/runtime material descriptor library shape for `.nemat` content.
///
/// `.nemat` is a material library with one or more addressable entries. It is
/// not a renderer pipeline, a shader binary, a texture dictionary, or a single
/// loose material blob. Runtime selection is `file.nemat@entry`.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(default))]
pub struct AuthoredMaterialLibrary {
    pub version: u32,
    pub materials: Vec<AuthoredMaterialDescriptor>,
}

impl Default for AuthoredMaterialLibrary {
    fn default() -> Self {
        Self {
            version: 1,
            materials: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(default))]
pub struct AuthoredMaterialDescriptor {
    pub name: String,
    pub shader: String,
    pub surface: AuthoredMaterialSurface,
    /// Semantic texture slots. Values must be `.ytd@entry` selectors.
    pub textures: BTreeMap<String, String>,
    /// Typed material parameters used by shader contracts and material tooling.
    pub params: BTreeMap<String, MaterialParamValue>,
}

impl Default for AuthoredMaterialDescriptor {
    fn default() -> Self {
        Self {
            name: String::new(),
            shader: "pbr.default".to_owned(),
            surface: AuthoredMaterialSurface::default(),
            textures: BTreeMap::new(),
            params: BTreeMap::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(default))]
pub struct AuthoredMaterialSurface {
    pub blend: String,
    pub two_sided: bool,
    pub alpha_cutoff: Option<f32>,
}

impl Default for AuthoredMaterialSurface {
    fn default() -> Self {
        Self {
            blend: "opaque".to_owned(),
            two_sided: false,
            alpha_cutoff: None,
        }
    }
}

/// Typed material parameter value. This replaces the old `BTreeMap<String, f32>`
/// shape so shader contracts and material editor UI can validate slots without
/// guessing intent from parameter names.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(tag = "type", content = "value", rename_all = "snake_case")
)]
pub enum MaterialParamValue {
    Float(f32),
    Float2([f32; 2]),
    Float3([f32; 3]),
    Float4([f32; 4]),
    Int(i32),
    Bool(bool),
    Color([f32; 4]),
    Enum(String),
    TextureRef(String),
}

impl Default for MaterialParamValue {
    fn default() -> Self {
        Self::Float(0.0)
    }
}

impl MaterialParamValue {
    #[inline]
    pub fn texture_ref(reference: impl Into<String>) -> Self {
        Self::TextureRef(reference.into())
    }
}

/// Binary/runtime material-library body projection. The actual NEF8 body can use
/// a compact table layout, but domain handlers must be able to project this
/// deterministic schema for tools, tests and AssetGraphResolver.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(default))]
pub struct NematMaterialLibraryBodyV1 {
    pub schema: String,
    /// Common ListFile entry projection shared with .ytd/.ydd/.ytyp.
    pub common_entries: Vec<ListFileEntryRecord>,
    /// Dependency table: material entries point to .ytd@entry texture refs.
    pub dependencies: Vec<AssetDependencyRecord>,
    pub metadata_namespaces: Vec<ListFileMetadataNamespace>,
    pub entries: Vec<MaterialEntryV1>,
    pub texture_bindings: Vec<MaterialTextureBindingV1>,
    pub params: BTreeMap<String, MaterialParamValue>,
    pub metadata: BTreeMap<String, String>,
}

impl Default for NematMaterialLibraryBodyV1 {
    fn default() -> Self {
        Self {
            schema: "newengine.nemat.material_library.v1".to_owned(),
            common_entries: Vec::new(),
            dependencies: Vec::new(),
            metadata_namespaces: Vec::new(),
            entries: Vec::new(),
            texture_bindings: Vec::new(),
            params: BTreeMap::new(),
            metadata: BTreeMap::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(default))]
pub struct MaterialEntryV1 {
    pub name: String,
    pub name_hash: u64,
    pub name_offset: u32,
    pub shader_id: String,
    pub domain: String,
    pub shading_model: String,
    pub descriptor_index: u32,
    pub texture_binding_range: std::ops::Range<u32>,
    pub param_range: std::ops::Range<u32>,
    pub flags: u32,
    pub metadata_range: std::ops::Range<u32>,
}

impl Default for MaterialEntryV1 {
    fn default() -> Self {
        Self {
            name: String::new(),
            name_hash: 0,
            name_offset: 0,
            shader_id: "pbr.default".to_owned(),
            domain: "surface".to_owned(),
            shading_model: "pbr_metallic_roughness".to_owned(),
            descriptor_index: 0,
            texture_binding_range: 0..0,
            param_range: 0..0,
            flags: 0,
            metadata_range: 0..0,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(default))]
pub struct MaterialTextureBindingV1 {
    pub slot_name: String,
    pub texture_ref: String,
    pub required: bool,
    pub color_space_policy: String,
    pub fallback_policy: String,
}

impl Default for MaterialTextureBindingV1 {
    fn default() -> Self {
        Self {
            slot_name: String::new(),
            texture_ref: String::new(),
            required: true,
            color_space_policy: "material_slot_default".to_owned(),
            fallback_policy: "error_if_required".to_owned(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(default))]
pub struct AuthoredMaterialValidation {
    pub valid: bool,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}
pub fn validate_authored_material_library(
    library: &AuthoredMaterialLibrary,
) -> AuthoredMaterialValidation {
    let mut result = AuthoredMaterialValidation::default();
    if library.version != 1 {
        result.errors.push(format!(
            "unsupported material library version {}; expected 1",
            library.version
        ));
    }
    if library.materials.is_empty() {
        result
            .warnings
            .push("material library contains no materials".to_owned());
    }
    let mut names = std::collections::BTreeSet::new();
    for material in &library.materials {
        let name = material.name.trim();
        if name.is_empty() {
            result
                .errors
                .push("material entry has empty name".to_owned());
        } else if !names.insert(name.to_ascii_lowercase()) {
            result
                .errors
                .push(format!("duplicate material name '{name}'"));
        }
        for (slot, reference) in &material.textures {
            if let Err(error) = validate_material_texture_reference(reference) {
                result.errors.push(format!(
                    "material '{}' texture slot '{}' invalid: {}",
                    material.name, slot, error
                ));
            }
        }
        for (param, value) in &material.params {
            if let MaterialParamValue::TextureRef(reference) = value {
                if let Err(error) = validate_material_texture_reference(reference) {
                    result.errors.push(format!(
                        "material '{}' param '{}' texture ref invalid: {}",
                        material.name, param, error
                    ));
                }
            }
        }
    }
    result.valid = result.errors.is_empty();
    result
}
