use super::*;

pub(super) fn publish_text_editor_state(
    mut patch: UiStatePatch,
    snapshot: &InspectorUiSnapshot<'_>,
) -> UiStatePatch {
    let text_lines = snapshot.text_lines.unwrap_or_default();
    let total_pages = text_lines.len().max(1).div_ceil(TEXT_ROWS);
    let page = snapshot.text_page.min(total_pages.saturating_sub(1));
    let start = page * TEXT_ROWS;
    let end = (start + TEXT_ROWS).min(text_lines.len());
    let visible_lines = &text_lines[start..end];

    patch = patch
        .with_change(
            "text_editor",
            "visible",
            json!(snapshot.text_lines.is_some()),
        )
        .with_change(
            "text_editor",
            "asset_ref",
            json!(snapshot.text_asset_ref.unwrap_or_default()),
        )
        .with_change("text_editor", "language", json!(snapshot.text_language))
        .with_change("text_editor", "editable", json!(snapshot.text_editable))
        .with_change("text_editor", "dirty", json!(snapshot.text_dirty))
        .with_change(
            "text_editor",
            "state_label",
            json!(if snapshot.text_dirty {
                "MODIFIED"
            } else {
                "CLEAN"
            }),
        )
        .with_change(
            "text_editor",
            "page_label",
            json!(format!("{} / {}", page + 1, total_pages)),
        )
        .with_change("text_editor", "can_previous", json!(page > 0))
        .with_change("text_editor", "can_next", json!(page + 1 < total_pages))
        .with_change(
            "text_editor",
            "can_save",
            json!(snapshot.text_editable && snapshot.text_dirty),
        );

    for row in 0..TEXT_ROWS {
        let source = format!("text_line_{row:02}");
        let highlighted = snapshot.syntax_editor.and_then(|page| page.rows.get(row));
        if let Some(line) = visible_lines.get(row) {
            patch = patch
                .with_change(&source, "visible", json!(true))
                .with_change(&source, "number", json!(start + row + 1))
                .with_change(&source, "value", json!(line))
                .with_change(&source, "editable", json!(snapshot.text_editable));
            for (layer, name) in SYNTAX_LAYER_NAMES.iter().enumerate() {
                patch = patch.with_change(
                    &source,
                    format!("syntax_{name}"),
                    json!(highlighted
                        .map(|row| row.layers[layer].as_str())
                        .unwrap_or_default()),
                );
            }
        } else {
            patch = patch
                .with_change(&source, "visible", json!(false))
                .with_change(&source, "number", json!(""))
                .with_change(&source, "value", json!(""))
                .with_change(&source, "editable", json!(false));
            for name in SYNTAX_LAYER_NAMES {
                patch = patch.with_change(&source, format!("syntax_{name}"), json!(""));
            }
        }
    }
    patch
}
