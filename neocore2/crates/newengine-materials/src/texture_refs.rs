#![forbid(unsafe_op_in_unsafe_fn)]

//! Canonical material texture reference handling.
//!
//! Runtime material textures are intentionally restricted to NewEngine texture
//! dictionaries. Source image containers (PNG/JPEG/DDS/etc.) are authoring inputs
//! for tools only and must not pass through material binding sanitization.

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MaterialTextureReference {
    pub dictionary_path: String,
    pub entry_selector: String,
    pub canonical: String,
}

impl MaterialTextureReference {
    #[inline]
    pub fn parse(value: &str) -> Option<Self> {
        let canonical = normalize_path(value);
        let (dictionary_path, entry_selector) = canonical.rsplit_once('@')?;
        let dictionary_path = dictionary_path.trim();
        let entry_selector = entry_selector.trim();
        if dictionary_path.is_empty() || entry_selector.is_empty() {
            return None;
        }
        if !dictionary_path.to_ascii_lowercase().ends_with(".neytd") {
            return None;
        }
        Some(Self {
            dictionary_path: dictionary_path.to_owned(),
            entry_selector: entry_selector.to_owned(),
            canonical,
        })
    }
}

#[inline]
pub fn normalize_material_texture_reference(value: &str) -> Option<String> {
    MaterialTextureReference::parse(value).map(|v| v.canonical)
}

#[inline]
pub fn is_material_texture_reference(value: &str) -> bool {
    MaterialTextureReference::parse(value).is_some()
}

fn normalize_path(value: &str) -> String {
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
