use std::collections::BTreeMap;

use crate::texture_refs::validate_material_texture_reference;

/// Authored material descriptor/library shape for `.nemat` content.
///
/// This is the source-facing layer. It is resolved through `engine.materials`
/// into `ResolvedMaterialGraph` and only then lowered into `RenderMaterialPacket`.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(default))]
pub struct AuthoredMaterialLibrary {
    pub version: u32,
    pub materials: Vec<AuthoredMaterialDescriptor>,
}

impl Default for AuthoredMaterialLibrary {
    fn default() -> Self { Self { version: 1, materials: Vec::new() } }
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
    pub params: BTreeMap<String, f32>,
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
    fn default() -> Self { Self { blend: "opaque".to_owned(), two_sided: false, alpha_cutoff: None } }
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(default))]
pub struct AuthoredMaterialValidation {
    pub valid: bool,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

impl Default for AuthoredMaterialValidation {
    fn default() -> Self { Self { valid: false, errors: Vec::new(), warnings: Vec::new() } }
}

pub fn validate_authored_material_library(library: &AuthoredMaterialLibrary) -> AuthoredMaterialValidation {
    let mut result = AuthoredMaterialValidation::default();
    if library.version != 1 {
        result.errors.push(format!("unsupported material library version {}; expected 1", library.version));
    }
    if library.materials.is_empty() {
        result.warnings.push("material library contains no materials".to_owned());
    }
    let mut names = std::collections::BTreeSet::new();
    for material in &library.materials {
        let name = material.name.trim();
        if name.is_empty() {
            result.errors.push("material entry has empty name".to_owned());
        } else if !names.insert(name.to_ascii_lowercase()) {
            result.errors.push(format!("duplicate material name '{name}'"));
        }
        for (slot, reference) in &material.textures {
            if let Err(error) = validate_material_texture_reference(reference) {
                result.errors.push(format!("material '{}' texture slot '{}' invalid: {}", material.name, slot, error));
            }
        }
    }
    result.valid = result.errors.is_empty();
    result
}
