use super::*;

pub(super) unsafe fn draw_inspector(hdc: Hdc, rect: Rect, state: &UiState) {
    let rows = sample_rows();
    if state.xml_path.is_some() {
        draw_xml_editor(hdc, rect, state);
        return;
    }
    if state.preview_path.is_some() {
        draw_asset_preview_editor(hdc, rect, state);
        return;
    }
    let mut y = rect.top + 44 - (state.inspector_scroll_rows as i32 * 24);
    draw_text(
        hdc,
        Rect {
            left: rect.left + 12,
            top: y,
            right: rect.right - 12,
            bottom: y + 22,
        },
        "Selected asset",
        rgb(20, 77, 138),
        true,
    );
    y += 26;
    if let Some(selected) = rows.get(state.selected_row).or_else(|| rows.first()) {
        draw_kv(hdc, rect, &mut y, "Name", selected.name);
        draw_kv(hdc, rect, &mut y, "Type", selected.kind);
        draw_kv(hdc, rect, &mut y, "Provider", selected.provider);
        draw_kv(hdc, rect, &mut y, "File size", &format_size(selected.size));
        draw_kv(hdc, rect, &mut y, "Path", selected.path);
    } else {
        draw_kv(hdc, rect, &mut y, "Selection", "No items in selected node");
        draw_kv(hdc, rect, &mut y, "Path", &state.selected_path);
    }
    y += 18;
    draw_text(
        hdc,
        Rect {
            left: rect.left + 12,
            top: y,
            right: rect.right - 12,
            bottom: y + 22,
        },
        "Runtime state",
        rgb(20, 77, 138),
        true,
    );
    y += 26;
    draw_kv(hdc, rect, &mut y, "Row", &state.selected_row.to_string());
    draw_kv(
        hdc,
        rect,
        &mut y,
        "List scroll",
        &state.scroll_rows.to_string(),
    );
    draw_kv(
        hdc,
        rect,
        &mut y,
        "Tree scroll",
        &state.tree_scroll_rows.to_string(),
    );
    draw_kv(
        hdc,
        rect,
        &mut y,
        "Inspector scroll",
        &state.inspector_scroll_rows.to_string(),
    );
    draw_kv(
        hdc,
        rect,
        &mut y,
        "Tree node",
        &state.selected_tree.to_string(),
    );
    y += 18;
    draw_text(
        hdc,
        Rect {
            left: rect.left + 12,
            top: y,
            right: rect.right - 12,
            bottom: y + 22,
        },
        "Format types",
        rgb(20, 77, 138),
        true,
    );
    y += 26;
    for type_id in state.format_type_ids.iter().take(8) {
        draw_text(
            hdc,
            Rect {
                left: rect.left + 18,
                top: y,
                right: rect.right - 12,
                bottom: y + 22,
            },
            &format!("- {type_id}"),
            rgb(55, 65, 81),
            false,
        );
        y += 22;
        if y > rect.bottom - 32 {
            break;
        }
    }
}
