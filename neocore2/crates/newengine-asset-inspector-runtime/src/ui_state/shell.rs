use super::*;

pub(super) fn publish_shell_state(
    patch: UiStatePatch,
    snapshot: &InspectorUiSnapshot<'_>,
    window_start: usize,
) -> UiStatePatch {
    let visible_count = snapshot
        .entries
        .len()
        .saturating_sub(window_start)
        .min(ENTRY_ROWS);
    let max_start = snapshot.entries.len().saturating_sub(ENTRY_ROWS);
    let offset_01 = if max_start == 0 {
        0.0
    } else {
        window_start as f32 / max_start as f32
    };
    let page_01 = if snapshot.entries.is_empty() {
        1.0
    } else {
        (visible_count as f32 / snapshot.entries.len() as f32).clamp(0.02, 1.0)
    };
    let range_label = if snapshot.entries.is_empty() {
        "0 / 0".to_owned()
    } else {
        format!(
            "{}-{} / {}",
            window_start + 1,
            window_start + visible_count,
            snapshot.entries.len()
        )
    };
    let details_visible = snapshot.preview.is_some();
    let content_width_px = if details_visible { 990.0 } else { 1564.0 };
    let browser_scroll_width_px = if details_visible { 966.0 } else { 1540.0 };
    let browser_row_width_px = if details_visible { 940.0 } else { 1514.0 };
    let browser_path_width_px = if details_visible { 454.0 } else { 1028.0 };

    patch
        .with_change(
            "shell",
            "path",
            json!(display_path(
                snapshot.current_path,
                snapshot.inside_container
            )),
        )
        .with_change(
            "shell",
            "location_kind",
            json!(if snapshot.inside_container {
                "PROVIDER MANIFEST"
            } else {
                "ASSET VFS"
            }),
        )
        .with_change(
            "shell",
            "inside_container",
            json!(snapshot.inside_container),
        )
        .with_change("shell", "mode", json!(snapshot.mode.label()))
        .with_change("shell", "status", json!(snapshot.status))
        .with_change(
            "shell",
            "activity_progress_01",
            json!(snapshot.activity_progress_01),
        )
        .with_change(
            "shell",
            "activity_width_px",
            json!(snapshot.activity_width_px),
        )
        .with_change("shell", "activity_label", json!(snapshot.activity_label))
        .with_change(
            "shell",
            "hover_hint",
            json!(if snapshot.hover_hint.trim().is_empty() {
                "Single-click selects | Double-click previews | Mouse wheel scrolls asset lists"
            } else {
                snapshot.hover_hint
            }),
        )
        .with_change("shell", "entry_count", json!(snapshot.entries.len()))
        .with_change(
            "shell",
            "browser_visible",
            json!(snapshot.text_lines.is_none()),
        )
        .with_change(
            "shell",
            "text_editor_visible",
            json!(snapshot.text_lines.is_some()),
        )
        .with_change(
            "shell",
            "info_modal_visible",
            json!(snapshot.info_modal_visible),
        )
        .with_change(
            "shell",
            "info_available",
            json!(snapshot.document.is_some()),
        )
        .with_change("shell", "details_visible", json!(details_visible))
        .with_change("shell", "content_width_px", json!(content_width_px))
        .with_change(
            "shell",
            "browser_scroll_width_px",
            json!(browser_scroll_width_px),
        )
        .with_change("shell", "browser_row_width_px", json!(browser_row_width_px))
        .with_change(
            "shell",
            "browser_path_width_px",
            json!(browser_path_width_px),
        )
        .with_change("shell", "browser_range_label", json!(range_label))
        .with_change("shell", "browser_scroll_offset_01", json!(offset_01))
        .with_change("shell", "browser_scroll_page_01", json!(page_01))
        .with_change(
            "shell",
            "browser_scroll_content_extent_px",
            json!((snapshot.entries.len().max(1) * 49) as f32),
        )
        .with_change(
            "shell",
            "browser_scrollbar_visible",
            json!(snapshot.entries.len() > ENTRY_ROWS),
        )
        .with_change(
            "shell",
            "can_up",
            json!(snapshot.inside_container || !snapshot.current_path.trim().is_empty()),
        )
        .with_change(
            "mode_all",
            "active",
            json!(snapshot.mode == AssetInspectorMode::All),
        )
        .with_change(
            "mode_assets",
            "active",
            json!(snapshot.mode == AssetInspectorMode::Assets),
        )
        .with_change(
            "mode_folders",
            "active",
            json!(snapshot.mode == AssetInspectorMode::Folders),
        )
}

pub(super) fn display_path(path: &str, inside_container: bool) -> String {
    if path.trim().is_empty() {
        return "engine.assets:/".to_owned();
    }
    if inside_container {
        format!("engine.assets:/{}#entries", path.trim_matches('/'))
    } else {
        format!("engine.assets:/{}", path.trim_matches('/'))
    }
}
