use super::*;

pub(super) fn icon_for_descriptor(
    descriptor: Option<&AssetFileTypeDescriptor>,
    extension: &str,
) -> &'static str {
    match descriptor
        .map(|d| d.asset_kind.as_str())
        .unwrap_or(extension)
    {
        "texture_dictionary" => "textures/ui/icons/assetBrowser.ytd@texture",
        "material_library" => "textures/ui/icons/assetBrowser.ytd@material",
        "drawable_dictionary" | "drawable" => "textures/ui/icons/assetBrowser.ytd@model",
        "archetype_dictionary" | "map_data" => "textures/ui/icons/assetBrowser.ytd@world",
        "asset_package" => "textures/ui/icons/assetBrowser.ytd@package",
        "ui_dictionary" => "textures/ui/icons/assetBrowser.ytd@ui",
        "font_dictionary" => "textures/ui/icons/assetBrowser.ytd@ui",
        "script_module" => "textures/ui/icons/assetBrowser.ytd@script",
        _ => "textures/ui/icons/assetBrowser.ytd@generic",
    }
}

pub(super) fn normalize_asset_ref(value: &str) -> String {
    let mut out = value.trim().replace('\\', "/");
    while let Some(rest) = out.strip_prefix("./") {
        out = rest.to_owned();
    }
    while out.contains("//") {
        out = out.replace("//", "/");
    }
    out.trim_start_matches('/').to_owned()
}

pub(super) fn split_entry_ref(asset_ref: &str) -> (String, Option<String>) {
    let mut parts = asset_ref.splitn(2, '@');
    let path = parts.next().unwrap_or_default().to_owned();
    let entry = parts
        .next()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToOwned::to_owned);
    (path, entry)
}

pub(super) fn path_extension(path: &str) -> String {
    path.rsplit('/')
        .next()
        .unwrap_or(path)
        .rsplit_once('.')
        .map(|(_, ext)| ext.to_ascii_lowercase())
        .unwrap_or_default()
}

pub(super) fn file_name(path: &str) -> String {
    path.rsplit('/')
        .next()
        .filter(|s| !s.is_empty())
        .unwrap_or(path)
        .to_owned()
}
