use super::*;

pub(super) fn publish_browser_entries(
    mut patch: UiStatePatch,
    snapshot: &InspectorUiSnapshot<'_>,
    start: usize,
    visible_entries: &[InspectorEntry],
) -> UiStatePatch {
    for row in 0..ENTRY_ROWS {
        let source = format!("entry_{row:02}");
        if let Some(entry) = visible_entries.get(row) {
            let absolute_index = start + row;
            patch = patch
                .with_change(&source, "visible", json!(true))
                .with_change(&source, "name", json!(entry.name))
                .with_change(&source, "marker", json!(entry.marker()))
                .with_change(&source, "detail", json!(entry.detail()))
                .with_change(
                    &source,
                    "path",
                    json!(
                        if entry.is_parent_navigation() && entry.logical_path.is_empty() {
                            "/"
                        } else {
                            entry.logical_path.as_str()
                        }
                    ),
                )
                .with_change(
                    &source,
                    "selected",
                    json!(snapshot.selected_index == Some(absolute_index)),
                );
        } else {
            patch = patch
                .with_change(&source, "visible", json!(false))
                .with_change(&source, "name", json!(""))
                .with_change(&source, "marker", json!(""))
                .with_change(&source, "detail", json!(""))
                .with_change(&source, "path", json!(""))
                .with_change(&source, "selected", json!(false));
        }
    }
    patch
}
