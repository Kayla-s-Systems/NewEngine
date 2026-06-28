//! Asset Browser labels and icon refs.

//!

//! Presentation-only layer. It may display provider-declared DTO facts, but it

//! must not infer semantic type, preview route or editor capability from file

//! extension, entry name or hash.

use crate::{AssetsCatalogEntry, ASSET_BROWSER_ICON_FOLDER, ASSET_BROWSER_ICON_GENERIC};

pub(crate) fn asset_type_label(entry: &AssetsCatalogEntry) -> String {
    if entry.is_directory() {
        return "Folder".to_owned();
    }

    let kind = entry.asset_kind.trim();

    if kind.is_empty() || kind == "asset" {
        "Asset".to_owned()
    } else {
        kind.to_owned()
    }
}

pub(crate) fn icon_for_entry(entry: &AssetsCatalogEntry) -> &'static str {
    if entry.is_directory() {
        ASSET_BROWSER_ICON_FOLDER
    } else {
        ASSET_BROWSER_ICON_GENERIC
    }
}

pub(crate) fn preview_plan_label(entry: &AssetsCatalogEntry) -> &'static str {
    if entry.is_directory() {
        "folder preview"
    } else {
        "declared provider contract required"
    }
}
