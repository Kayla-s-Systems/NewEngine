use super::*;

#[derive(Debug, Clone)]
pub(super) struct AssetRouteResolution {
    pub(super) is_registered: bool,
    pub(super) provider_label: String,
    pub(super) tool_route: Option<ToolRouteDescriptor>,
    pub(super) type_id: Option<String>,
    pub(super) content_kind: Option<String>,
    pub(super) preview_surface: Option<String>,
}

pub(super) fn resolve_asset_route(state: &UiState, path: &Path) -> AssetRouteResolution {
    let extension_key = normalized_extension(path);
    if let Some(tool_route) = registered_tool_route_for_extension(state, &extension_key) {
        return AssetRouteResolution {
            is_registered: true,
            provider_label: tool_route.provider_id.clone(),
            tool_route: Some(tool_route),
            type_id: builtin_type_id_for_path(path),
            content_kind: builtin_content_kind_for_path(path),
            preview_surface: builtin_preview_surface_for_path(path),
        };
    }
    AssetRouteResolution {
        is_registered: is_builtin_preview_route(&extension_key, &path.display().to_string()),
        provider_label: preview_provider_for_extension(path),
        tool_route: None,
        type_id: builtin_type_id_for_path(path),
        content_kind: builtin_content_kind_for_path(path),
        preview_surface: builtin_preview_surface_for_path(path),
    }
}

pub(super) fn is_text_editor_surface(route: &AssetRouteResolution) -> bool {
    matches!(
        route.preview_surface.as_deref(),
        Some("text") | Some("xml_highlight")
    ) || route.content_kind.as_deref().is_some_and(|kind| {
        kind.contains("document") || kind.contains("script") || kind.contains("source")
    })
}

pub(super) fn builtin_type_id_for_path(path: &Path) -> Option<String> {
    builtin_content_kind_for_path(path).map(|kind| format!("asset.type.{kind}"))
}

pub(super) fn builtin_content_kind_for_path(path: &Path) -> Option<String> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    let kind = match ext.as_str() {
        "xml" => "xml_document",
        "json" => "json_document",
        "toml" => "toml_document",
        "lua" => "lua_script",
        "hlsl" | "glsl" => "shader_source",
        "rs" | "py" | "cpp" | "c" | "hpp" | "h" | "cs" => "source_document",
        "txt" | "md" | "markdown" | "ini" | "cfg" | "log" | "yaml" | "yml" => "text_document",
        _ => return None,
    };
    Some(kind.to_owned())
}

pub(super) fn builtin_preview_surface_for_path(path: &Path) -> Option<String> {
    let kind = builtin_content_kind_for_path(path)?;
    if kind == "xml_document" {
        Some("xml_highlight".to_owned())
    } else {
        Some("text".to_owned())
    }
}

pub(super) fn is_builtin_preview_route(extension: &str, path: &str) -> bool {
    let lower_path = path.to_ascii_lowercase();
    lower_path.ends_with(".nemat.xml")
        || lower_path.ends_with(".neui.xml")
        || lower_path.ends_with(".ytyp.xml")
        || matches!(
            extension,
            ".xml"
                | ".txt"
                | ".md"
                | ".json"
                | ".toml"
                | ".ini"
                | ".cfg"
                | ".log"
                | ".rs"
                | ".py"
                | ".cpp"
                | ".c"
                | ".hpp"
                | ".h"
                | ".cs"
                | ".png"
                | ".jpg"
                | ".jpeg"
                | ".webp"
                | ".bmp"
                | ".tga"
                | ".dds"
                | ".obj"
                | ".gltf"
                | ".glb"
                | ".fbx"
        )
}

pub(super) fn normalized_extension(path: &Path) -> String {
    path.extension()
        .and_then(|value| value.to_str())
        .map(|extension| format!(".{}", extension.to_ascii_lowercase()))
        .unwrap_or_default()
}

pub(super) fn registered_tool_route_for_extension(
    state: &UiState,
    extension: &str,
) -> Option<ToolRouteDescriptor> {
    let normalized = extension.to_ascii_lowercase();
    state
        .tool_routes
        .iter()
        .find(|route| route.extension.eq_ignore_ascii_case(&normalized))
        .cloned()
}

pub(super) fn preview_surface_for_kind(kind: &str, path: &str) -> &'static str {
    let lower_path = path.to_ascii_lowercase();
    if lower_path.ends_with(".ytd") || kind.eq_ignore_ascii_case(".ytd") {
        "Texture dictionary preview"
    } else if lower_path.ends_with(".xml") {
        "XML syntax editor"
    } else if matches!(kind, "Text" | ".txt" | ".md" | ".json" | ".toml") {
        "Text editor"
    } else if matches!(kind, ".png" | ".jpg" | ".jpeg" | ".webp" | ".bmp" | ".tga") {
        "Image preview"
    } else if matches!(kind, ".obj" | ".gltf" | ".glb" | ".fbx" | ".ydd") {
        "3D model preview"
    } else if matches!(kind, "Package" | ".nepak" | ".rpf") {
        "Package browser"
    } else {
        "Provider preview"
    }
}
