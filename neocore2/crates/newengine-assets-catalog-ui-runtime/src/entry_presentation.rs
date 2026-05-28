//! Asset Browser labels and icon refs.
//!
//! This module is presentation-only. It maps already reported VFS/file-type DTO
//! data into labels consumed by `engine.ui`; it does not decide asset semantics.

use crate::{
    AssetsCatalogEntry, ASSET_BROWSER_ICON_AUDIO, ASSET_BROWSER_ICON_FOLDER,
    ASSET_BROWSER_ICON_GENERIC, ASSET_BROWSER_ICON_MATERIAL, ASSET_BROWSER_ICON_MODEL,
    ASSET_BROWSER_ICON_PACKAGE, ASSET_BROWSER_ICON_SCRIPT, ASSET_BROWSER_ICON_SHADER,
    ASSET_BROWSER_ICON_TEXTURE, ASSET_BROWSER_ICON_UI, ASSET_BROWSER_ICON_WORLD,
};

pub(crate) fn asset_type_label(entry: &AssetsCatalogEntry) -> String {
    if entry.is_directory() {
        return "Folder".to_owned();
    }
    let kind = entry.asset_kind.trim();
    if kind.is_empty() || kind == "asset" {
        match entry.extension.as_str() {
            "neui" => "UI Dictionary".to_owned(),
            "nemat" => "Material Library".to_owned(),
            "ytd" => "Texture Dictionary".to_owned(),
            "ydd" | "ydr" | "obj" | "gltf" | "glb" => "Model / Drawable".to_owned(),
            "ytyp" => "Scene Definition".to_owned(),
            "ymap" => "Map".to_owned(),
            "wav" | "ogg" => "Audio".to_owned(),
            _ => "Asset".to_owned(),
        }
    } else {
        kind.to_owned()
    }
}

pub(crate) fn icon_for_entry(entry: &AssetsCatalogEntry) -> &'static str {
    if entry.is_directory() { ASSET_BROWSER_ICON_FOLDER } else { icon_for_extension(&entry.extension) }
}

pub(crate) fn preview_plan_label(entry: &AssetsCatalogEntry) -> &'static str {
    if entry.is_directory() {
        "folder preview"
    } else {
        match entry.extension.as_str() {
            "ytd" | "png" | "jpg" | "jpeg" | "dds" => "texture preview provider",
            "nemat" => "material preview provider",
            "ydd" | "ydr" | "obj" | "gltf" | "glb" => "model preview provider",
            "ytyp" | "ymap" => "world metadata preview provider",
            "neui" => "UI preview provider",
            _ => "metadata preview provider",
        }
    }
}

pub(crate) fn icon_for_extension(ext: &str) -> &'static str {
    match ext.trim_start_matches('.').to_ascii_lowercase().as_str() {
        "neui" => ASSET_BROWSER_ICON_UI,
        "ytd" | "png" | "jpg" | "jpeg" | "dds" => ASSET_BROWSER_ICON_TEXTURE,
        "ydd" | "ydr" | "obj" | "gltf" | "glb" => ASSET_BROWSER_ICON_MODEL,
        "ytyp" | "ymap" => ASSET_BROWSER_ICON_WORLD,
        "nemat" => ASSET_BROWSER_ICON_MATERIAL,
        "nepak" => ASSET_BROWSER_ICON_PACKAGE,
        "nepat" => ASSET_BROWSER_ICON_GENERIC,
        "lua" | "ron" | "json" | "toml" | "rs" | "py" | "bat" | "cmd" => ASSET_BROWSER_ICON_SCRIPT,
        "vert" | "frag" | "wgsl" | "glsl" => ASSET_BROWSER_ICON_SHADER,
        "wav" | "ogg" => ASSET_BROWSER_ICON_AUDIO,
        "" => ASSET_BROWSER_ICON_GENERIC,
        _ => ASSET_BROWSER_ICON_GENERIC,
    }
}
