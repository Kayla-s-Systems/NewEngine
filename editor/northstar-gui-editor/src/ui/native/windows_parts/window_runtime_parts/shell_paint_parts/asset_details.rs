use super::*;

pub(super) unsafe fn draw_asset_details_panel(hdc: Hdc, rect: Rect, state: &UiState) {
    let mut y = rect.top + 44 - (state.inspector_scroll_rows as i32 * 24);
    if state.preview_path.is_some() || state.xml_path.is_some() {
        draw_text(
            hdc,
            Rect {
                left: rect.left + 12,
                top: y,
                right: rect.right - 12,
                bottom: y + 22,
            },
            "Opened Asset",
            rgb(20, 77, 138),
            true,
        );
        y += 28;
        draw_kv(hdc, rect, &mut y, "Name", &state.preview_name);
        draw_kv(hdc, rect, &mut y, "Type", &state.preview_kind);
        draw_kv(hdc, rect, &mut y, "Provider", &state.preview_provider);
        draw_kv(
            hdc,
            rect,
            &mut y,
            "File size",
            &format_size(state.preview_size),
        );
        if let Some(path) = &state.preview_path {
            draw_kv(hdc, rect, &mut y, "Path", path);
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
            "Controls",
            rgb(20, 77, 138),
            true,
        );
        y += 26;
        draw_kv(hdc, rect, &mut y, "Back", "Toolbar Back or Escape");
        draw_kv(
            hdc,
            rect,
            &mut y,
            "Save",
            if state.xml_path.is_some() {
                "enabled for XML buffer"
            } else {
                "provider write route required"
            },
        );
        draw_kv(
            hdc,
            rect,
            &mut y,
            "Preview",
            preview_surface_for_kind(
                &state.preview_kind,
                state.preview_path.as_deref().unwrap_or_default(),
            ),
        );
        return;
    }
    draw_inspector(hdc, rect, state);
}
