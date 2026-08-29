use super::*;

impl AssetInspectorRuntimeModule {}

pub(super) fn provider_label(document: &AssetDocument) -> &str {
    if document.provider_service.trim().is_empty() {
        document.semantic_gateway.as_str()
    } else {
        document.provider_service.as_str()
    }
}
pub(super) fn opened_asset_status(
    document: &AssetDocument,
    preview: &AssetPreviewSnapshot,
    container_available: bool,
    container_entry_count: usize,
) -> String {
    if let Some(text) = document.text.as_ref() {
        return format!(
            "Opened {} | TEXT EDITOR | syntax={} | editable={}",
            document.title, text.language, text.editable
        );
    }
    let source = document
        .asset_ref
        .split('@')
        .next()
        .unwrap_or(&document.asset_ref);
    let extension = source
        .rsplit_once('.')
        .map(|(_, extension)| extension.to_ascii_lowercase())
        .unwrap_or_default();
    match preview.kind {
        AssetPreviewKind::Texture2d => format!(
            "Opened {} | 2D TEXTURE PREVIEW | {}x{}",
            document.title, preview.width, preview.height
        ),
        AssetPreviewKind::Scene3d if extension == "nemat" => format!(
            "Opened {} | MATERIAL SPHERE PREVIEW | {}x{}",
            document.title, preview.width, preview.height
        ),
        AssetPreviewKind::Scene3d => format!(
            "Opened {} | 3D MODEL PREVIEW | {}x{}",
            document.title, preview.width, preview.height
        ),
        AssetPreviewKind::None if container_available => format!(
            "Opened {} | CONTAINER PREVIEW{} | entries are listed below",
            document.title,
            if container_entry_count == 0 {
                String::new()
            } else {
                format!(" | {} entries", container_entry_count)
            }
        ),
        AssetPreviewKind::None => format!(
            "Opened {} | preview unavailable | {}",
            document.title,
            preview
                .diagnostic
                .as_deref()
                .unwrap_or("provider returned no visual representation")
        ),
    }
}
pub(super) fn source_asset_ref(asset_ref: &str) -> &str {
    asset_ref.split('@').next().unwrap_or(asset_ref)
}

pub(super) fn preview_entry_selection(
    entries: &[InspectorEntry],
    asset_ref: &str,
    fallback: Option<usize>,
) -> Option<usize> {
    entries
        .iter()
        .position(|entry| entry.logical_path == asset_ref)
        .or_else(|| fallback.filter(|index| *index < entries.len()))
        .or_else(|| (!entries.is_empty()).then_some(0))
}

pub(super) fn document_exposes_entries(document: &AssetDocument) -> bool {
    let descriptor_exposes = document.descriptor.as_ref().is_some_and(|descriptor| {
        descriptor.native_container
            || descriptor.allow_nested_assets
            || matches!(
                descriptor.codec_type.trim(),
                "containerType" | "listType" | "listFile"
            )
            || descriptor.selector_syntax.is_some()
    });
    if descriptor_exposes || document.content_kind.is_some() {
        return true;
    }
    matches!(
        source_asset_ref(&document.asset_ref)
            .rsplit_once('.')
            .map(|(_, extension)| extension.to_ascii_lowercase())
            .as_deref(),
        Some(
            "ydd"
                | "ydr"
                | "ytd"
                | "nemat"
                | "ytyp"
                | "ymap"
                | "ybn"
                | "yft"
                | "ymt"
                | "ycd"
                | "yed"
                | "neui"
                | "nepak"
        )
    )
}
