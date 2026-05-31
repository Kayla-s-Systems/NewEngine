#![forbid(unsafe_op_in_unsafe_fn)]

//! Canonical material texture reference handling.
//!
//! Authored/runtime material graphs reference texture dictionaries through the
//! shared VFS syntax `<logical-path>[@entry]`. Source image containers
//! (PNG/JPG/TGA/DDS/etc.) are import inputs for tools only and must not appear in
//! material graphs. 

use newengine_assets_api::{
    is_raw_source_image_reference, is_retired_texture_dictionary_reference, require_asset_reference_extension,
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
        if is_retired_texture_dictionary_reference(value) {
            return Err("material texture references must use .ytd@entry; non-canonical texture dictionaries are not public material texture references".to_owned());
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


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_raw_source_image_texture_refs() {
        assert!(validate_material_texture_reference("foo.png").is_err());
        assert!(validate_material_texture_reference("foo.dds").is_err());
        assert!(validate_material_texture_reference("foo.jpg").is_err());
    }

    #[test]
    fn rejects_ytd_without_entry() {
        assert!(validate_material_texture_reference("textures/world.ytd").is_err());
    }

    #[test]
    fn accepts_ytd_entry() {
        let reference = validate_material_texture_reference("textures/world.ytd@brick_albedo").unwrap();
        assert_eq!(reference.dictionary_path, "textures/world.ytd");
        assert_eq!(reference.entry_selector, "brick_albedo");
        assert_eq!(reference.canonical, "textures/world.ytd@brick_albedo");
    }
}
