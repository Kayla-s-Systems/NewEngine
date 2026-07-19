use super::*;

pub(super) fn publish_preview_state(
    mut patch: UiStatePatch,
    document: Option<&AssetDocument>,
    preview: Option<&AssetPreviewSnapshot>,
    syntax_preview: Option<&SyntaxPreviewPage>,
    pointer_captured: bool,
    container_available: bool,
) -> UiStatePatch {
    let image_ready = preview.is_some_and(|preview| {
        preview.ready
            && matches!(
                preview.kind,
                AssetPreviewKind::Texture2d | AssetPreviewKind::Scene3d
            )
    });
    let text_ready = syntax_preview.is_some();
    let scene_3d = preview.is_some_and(|preview| preview.kind == AssetPreviewKind::Scene3d);
    let visible = image_ready || text_ready || container_available;
    let ready = image_ready || text_ready;
    let texture_ref = preview
        .and_then(|preview| preview.texture_ref.as_deref())
        .unwrap_or_default();
    let ui_texture_id = preview
        .and_then(|preview| preview.ui_texture_id)
        .unwrap_or_default();
    let kind = if let Some(syntax) = syntax_preview {
        format!("TEXT | {}", syntax.language.to_ascii_uppercase())
    } else {
        preview
            .map(|preview| match preview.kind {
                AssetPreviewKind::None => String::new(),
                AssetPreviewKind::Texture2d => "2D".to_owned(),
                AssetPreviewKind::Scene3d => "3D".to_owned(),
            })
            .unwrap_or_default()
    };
    let detail = if let Some(syntax) = syntax_preview {
        syntax.page_label()
    } else {
        preview
            .filter(|preview| preview.ready)
            .map(|preview| {
                if preview.width > 0 && preview.height > 0 {
                    format!("{} x {}", preview.width, preview.height)
                } else if preview.kind == AssetPreviewKind::Texture2d {
                    "provider texture".to_owned()
                } else {
                    String::new()
                }
            })
            .unwrap_or_default()
    };
    patch = patch
        .with_change("preview", "visible", json!(visible))
        .with_change("preview", "ready", json!(ready))
        .with_change(
            "preview",
            "title",
            json!(document
                .map(|document| document.title.as_str())
                .unwrap_or_default()),
        )
        .with_change("preview", "kind_label", json!(kind))
        .with_change("preview", "detail", json!(detail))
        .with_change("preview", "texture_ref", json!(texture_ref))
        .with_change("preview", "ui_texture_id", json!(ui_texture_id))
        .with_change("preview", "image_visible", json!(image_ready))
        .with_change("preview", "text_visible", json!(text_ready))
        .with_change("preview", "controls_visible", json!(scene_3d))
        .with_change("preview", "pointer_captured", json!(pointer_captured))
        .with_change(
            "preview",
            "controls_label",
            json!(if pointer_captured {
                "CAMERA INPUT ACTIVE | LMB ORBIT | MMB PAN"
            } else {
                "LMB ORBIT | MMB PAN | WHEEL ZOOM"
            }),
        )
        .with_change(
            "preview",
            "placeholder_visible",
            json!(!image_ready && !text_ready),
        );

    for row in 0..SYNTAX_PREVIEW_ROWS {
        let row_prefix = format!("row_{row:02}");
        if let Some(highlighted) = syntax_preview.and_then(|preview| preview.rows.get(row)) {
            patch = patch
                .with_change(
                    "syntax_preview",
                    format!("{row_prefix}_visible"),
                    json!(true),
                )
                .with_change(
                    "syntax_preview",
                    format!("{row_prefix}_number"),
                    json!(highlighted.line_number),
                );
            for (layer, name) in SYNTAX_LAYER_NAMES.iter().enumerate() {
                patch = patch.with_change(
                    "syntax_preview",
                    format!("{row_prefix}_{name}"),
                    json!(highlighted.layers[layer]),
                );
            }
        } else {
            patch = patch
                .with_change(
                    "syntax_preview",
                    format!("{row_prefix}_visible"),
                    json!(false),
                )
                .with_change("syntax_preview", format!("{row_prefix}_number"), json!(""));
            for name in SYNTAX_LAYER_NAMES {
                patch =
                    patch.with_change("syntax_preview", format!("{row_prefix}_{name}"), json!(""));
            }
        }
    }
    patch
}

pub(super) fn publish_preview_entry_state(
    mut patch: UiStatePatch,
    entries: &[InspectorEntry],
    selected: Option<usize>,
    requested_start: usize,
    loading: bool,
    available: bool,
) -> UiStatePatch {
    let max_start = entries.len().saturating_sub(PREVIEW_ENTRY_ROWS);
    let start = requested_start.min(max_start);
    let end = (start + PREVIEW_ENTRY_ROWS).min(entries.len());
    let visible_entries = &entries[start..end];
    let visible_count = visible_entries.len();
    let offset_01 = if max_start == 0 {
        0.0
    } else {
        start as f32 / max_start as f32
    };
    let page_01 = if entries.is_empty() {
        1.0
    } else {
        (visible_count as f32 / entries.len() as f32).clamp(0.02, 1.0)
    };
    patch = patch
        .with_change("preview_entries", "visible", json!(available))
        .with_change("preview_entries", "loading", json!(loading))
        .with_change("preview_entries", "count", json!(entries.len()))
        .with_change(
            "preview_entries",
            "range_label",
            json!(if entries.is_empty() {
                "0 / 0".to_owned()
            } else {
                format!(
                    "{}-{} / {}",
                    start + 1,
                    start + visible_count,
                    entries.len()
                )
            }),
        )
        .with_change("preview_entries", "scroll_offset_01", json!(offset_01))
        .with_change("preview_entries", "scroll_page_01", json!(page_01))
        .with_change(
            "preview_entries",
            "scroll_content_extent_px",
            json!((entries.len().max(1) * 34) as f32),
        )
        .with_change(
            "preview_entries",
            "scrollbar_visible",
            json!(entries.len() > PREVIEW_ENTRY_ROWS),
        )
        .with_change(
            "preview_entries",
            "title",
            json!(if loading {
                "ENTRIES | LOADING".to_owned()
            } else {
                format!("ENTRIES | {}", entries.len())
            }),
        )
        .with_change(
            "preview_entries",
            "empty_visible",
            json!(available && !loading && entries.is_empty()),
        )
        .with_change(
            "preview_entries",
            "empty_text",
            json!(if loading {
                "Resolving provider manifest"
            } else {
                "Provider exposed no addressable entries"
            }),
        );

    for row in 0..PREVIEW_ENTRY_ROWS {
        let source = format!("preview_entry_{row:02}");
        if let Some(entry) = visible_entries.get(row) {
            let absolute = start + row;
            patch = patch
                .with_change(&source, "visible", json!(true))
                .with_change(&source, "selected", json!(selected == Some(absolute)))
                .with_change(&source, "marker", json!(entry.marker()))
                .with_change(&source, "name", json!(entry.name))
                .with_change(
                    &source,
                    "kind",
                    json!(if entry.asset_kind.trim().is_empty() {
                        entry.kind.as_str()
                    } else {
                        entry.asset_kind.as_str()
                    }),
                )
                .with_change(&source, "size", json!(entry_size_label(entry.byte_len)));
        } else {
            patch = patch
                .with_change(&source, "visible", json!(false))
                .with_change(&source, "selected", json!(false))
                .with_change(&source, "marker", json!(""))
                .with_change(&source, "name", json!(""))
                .with_change(&source, "kind", json!(""))
                .with_change(&source, "size", json!(""));
        }
    }
    patch
}

pub(super) fn entry_size_label(byte_len: Option<u64>) -> String {
    let Some(bytes) = byte_len else {
        return "-".to_owned();
    };
    if bytes >= 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else if bytes >= 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{bytes} B")
    }
}
