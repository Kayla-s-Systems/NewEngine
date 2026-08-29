use super::*;

pub(crate) fn filtered_entry_indices(
    snapshot: &AssetsCatalogSnapshot,
    view_mode: CatalogViewMode,
    search_query: &str,
    collapsed_paths: &BTreeSet<String>,
) -> Vec<usize> {
    let query = search_query.trim().to_ascii_lowercase();
    snapshot
        .entries
        .iter()
        .enumerate()
        .filter(|(_, entry)| {
            if view_mode == CatalogViewMode::Tree && !entry.is_directory() {
                return false;
            }
            if entry.is_directory() && is_hidden_by_collapsed_parent(entry, collapsed_paths) {
                return false;
            }
            if query.is_empty() {
                return true;
            }
            entry.name.to_ascii_lowercase().contains(&query)
                || entry.logical_path.to_ascii_lowercase().contains(&query)
                || entry.asset_kind.to_ascii_lowercase().contains(&query)
                || entry.semantic_gateway.to_ascii_lowercase().contains(&query)
                || entry.extension.to_ascii_lowercase().contains(&query)
        })
        .map(|(index, _)| index)
        .collect()
}

pub(crate) fn is_hidden_by_collapsed_parent(
    entry: &AssetsCatalogEntry,
    collapsed_paths: &BTreeSet<String>,
) -> bool {
    let path = normalize_catalog_path(&entry.logical_path);
    collapsed_paths.iter().any(|collapsed| {
        let collapsed = normalize_catalog_path(collapsed);
        !collapsed.is_empty() && path != collapsed && path.starts_with(&(collapsed + "/"))
    })
}

pub(crate) fn main_visible_window_size(
    geometry: &CatalogWorkspaceGeometry,
    view_mode: CatalogViewMode,
) -> usize {
    let row_h = match view_mode {
        CatalogViewMode::Grid => 106.0,
        CatalogViewMode::Inspector => 30.0,
        CatalogViewMode::Tree | CatalogViewMode::List => 28.0,
    };
    let rows = (geometry.content_h / row_h).floor().max(1.0) as usize;
    let cols = match view_mode {
        CatalogViewMode::Grid => (geometry.main_w / 138.0).floor().max(1.0) as usize,
        CatalogViewMode::Inspector | CatalogViewMode::Tree | CatalogViewMode::List => 1,
    };
    rows.saturating_mul(cols).max(1)
}

pub(crate) fn visible_window_start(total: usize, selected_index: usize, window: usize) -> usize {
    if total <= window {
        return 0;
    }
    let half = window / 2;
    selected_index
        .saturating_sub(half)
        .min(total.saturating_sub(window))
}
