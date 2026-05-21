#![forbid(unsafe_op_in_unsafe_fn)]

//! Canonical material texture reference handling.
//!
//! Authored/runtime material graphs reference texture dictionaries through the
//! shared VFS syntax `<logical-path>[@entry]`. Source image containers
//! (PNG/JPG/TGA/DDS/etc.) are import inputs for tools only and must not appear in
//! material graphs. `.neytd` is legacy/cache compatibility and is rejected by the
//! public material contract.

use newengine_assets_api::{
    is_legacy_neytd_reference, is_raw_source_image_reference, require_asset_reference_extension,
    AssetReference,
};

pub const MATERIAL_TEXTURE_DICTIONARY_EXTENSION: &str = "ytd";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MaterialTextureReference {
    pub dictionary_path: String,
    pub entry_selector: String,
    pub canonical: String,
}

impl MaterialTextureReference {
    #[inline]
    pub fn parse(value: &str) -> Option<Self> { Self::parse_strict(value).ok() }

    pub fn parse_strict(value: &str) -> Result<Self, String> {
        if is_legacy_neytd_reference(value) {
            return Err("material texture references must use .ytd@entry; .neytd is legacy/cache-only".to_owned());
        }
        if is_raw_source_image_reference(value) {
            return Err("material texture references must use .ytd@entry; raw source image formats are import inputs only".to_owned());
        }
        let reference: AssetReference = require_asset_reference_extension(value, &[MATERIAL_TEXTURE_DICTIONARY_EXTENSION], true)?;
        let entry_selector = reference.entry.clone().unwrap_or_default();
        Ok(Self { dictionary_path: reference.logical_path, entry_selector, canonical: reference.canonical })
    }
}

#[inline]
pub fn normalize_material_texture_reference(value: &str) -> Option<String> {
    MaterialTextureReference::parse(value).map(|v| v.canonical)
}

#[inline]
pub fn validate_material_texture_reference(value: &str) -> Result<MaterialTextureReference, String> {
    MaterialTextureReference::parse_strict(value)
}

#[inline]
pub fn is_material_texture_reference(value: &str) -> bool { MaterialTextureReference::parse(value).is_some() }
