#![forbid(unsafe_op_in_unsafe_fn)]

//! Source material parser.
//!
//! This module owns human-authored material JSON. Scene/game bootstrap code should
//! request named material assets and then apply `MaterialRef` to objects; it should
//! not parse ad-hoc texture/color fields inline.

use crate::api::{MaterialAssetDocument, MaterialDescriptor, MaterialTextureBindings};

/// Parsed, renderer-agnostic material source asset.
#[derive(Clone, Debug, PartialEq)]
pub struct MaterialSourceDocument {
    pub name: Option<String>,
    pub desc: MaterialDescriptor,
    pub textures: MaterialTextureBindings,
}

impl MaterialSourceDocument {
    #[inline]
    pub fn new(
        name: Option<String>,
        mut desc: MaterialDescriptor,
        textures: MaterialTextureBindings,
    ) -> Self {
        desc.sanitize_in_place();
        Self {
            name: normalize_name(name),
            desc,
            textures: textures.sanitized(),
        }
    }

    #[inline]
    pub fn with_fallback_name(mut self, fallback: impl Into<String>) -> Self {
        if self.name.as_ref().map(|v| v.trim().is_empty()).unwrap_or(true) {
            self.name = normalize_name(Some(fallback.into()));
        }
        self
    }
}

#[derive(Debug, serde::Deserialize)]
#[serde(default)]
struct RawMaterialSourceDocument {
    /// Stable registry name, for example `materials/fps/terrain`.
    name: Option<String>,

    /// Preferred nested descriptor field.
    material: Option<MaterialDescriptor>,

    /// Accepted alias for tooling/exporters.
    descriptor: Option<MaterialDescriptor>,

    /// Preferred nested texture bindings.
    textures: Option<MaterialTextureBindings>,

    /// Backward-compatible flat material file support.
    #[serde(flatten)]
    flat: MaterialAssetDocument,
}

impl Default for RawMaterialSourceDocument {
    #[inline]
    fn default() -> Self {
        Self {
            name: None,
            material: None,
            descriptor: None,
            textures: None,
            flat: MaterialAssetDocument::default(),
        }
    }
}

impl RawMaterialSourceDocument {
    #[inline]
    fn into_source(self) -> MaterialSourceDocument {
        let desc = self.material.or(self.descriptor).unwrap_or(self.flat.desc);
        let textures = self.textures.unwrap_or(self.flat.textures);
        MaterialSourceDocument::new(self.name, desc, textures)
    }
}

/// Parse a material source JSON document.
///
/// Supported forms:
/// - flat legacy material: `{ "base_color": ..., "base_color_texture": ... }`
/// - source asset: `{ "name": "materials/foo", "material": {...}, "textures": {...} }`
#[inline]
pub fn parse_material_source_json(json: &str) -> Result<MaterialSourceDocument, serde_json::Error> {
    let raw = serde_json::from_str::<RawMaterialSourceDocument>(json)?;
    Ok(raw.into_source())
}

#[inline]
pub fn parse_material_source_slice(bytes: &[u8]) -> Result<MaterialSourceDocument, serde_json::Error> {
    let raw = serde_json::from_slice::<RawMaterialSourceDocument>(bytes)?;
    Ok(raw.into_source())
}

#[inline]
pub fn material_source_from_parts(
    name: impl Into<String>,
    desc: MaterialDescriptor,
    textures: MaterialTextureBindings,
) -> MaterialSourceDocument {
    MaterialSourceDocument::new(Some(name.into()), desc, textures)
}

#[inline]
fn normalize_name(name: Option<String>) -> Option<String> {
    let value = name?.trim().replace('\\', "/");
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}
