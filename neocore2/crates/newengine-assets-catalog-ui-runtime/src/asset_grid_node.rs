use super::*;

pub(crate) fn assets_catalog_node(
    frame_index: u64,
    surface_size_px: [u32; 2],
    snapshot: &AssetsCatalogSnapshot,
    selected_index: usize,
    hovered_entry_index: Option<usize>,
    view_mode: CatalogViewMode,
    search_query: &str,
    collapsed_paths: &BTreeSet<String>,
    focus_scope: CatalogFocusScope,
    document_actions: &[AssetDocumentAction],
    last_action_result: Option<&AssetPatchResult>,
    context_menu_open: bool,
) -> UiSurfaceNode {
    let folder_count = snapshot
        .entries
        .iter()
        .filter(|entry| entry.is_directory())
        .count();
    let asset_count = snapshot.entries.len().saturating_sub(folder_count);
    let visible_indices =
        filtered_entry_indices(snapshot, view_mode, search_query, collapsed_paths);
    let selected_entry = snapshot
        .entries
        .get(selected_index)
        .or_else(|| {
            visible_indices
                .first()
                .and_then(|idx| snapshot.entries.get(*idx))
        })
        .or_else(|| snapshot.entries.first());
    let geometry = catalog_workspace_geometry(surface_size_px);

    let body_lines = asset_grid_body_lines(
        snapshot,
        folder_count,
        asset_count,
        visible_indices.len(),
        focus_scope,
        search_query,
    );

    let mut components = Vec::new();
    push_catalog_regions(&mut components, &geometry);
    push_asset_grid_navigation_components(
        &mut components,
        snapshot,
        view_mode,
        search_query,
        focus_scope,
        document_actions,
        collapsed_paths,
        hovered_entry_index,
        &visible_indices,
    );

    let selected_slot = visible_indices
        .iter()
        .position(|idx| *idx == selected_index)
        .unwrap_or(0);
    let window_size = main_visible_window_size(&geometry, view_mode)
        .min(MAX_VISIBLE_ENTRIES)
        .min(visible_indices.len())
        .max(1);
    let window_start = visible_window_start(visible_indices.len(), selected_slot, window_size);
    let scroll_page_01 =
        (window_size as f32 / visible_indices.len().max(1) as f32).clamp(0.05, 1.0);
    let scroll_offset_01 = if visible_indices.len() <= window_size {
        0.0
    } else {
        window_start as f32 / visible_indices.len().saturating_sub(window_size).max(1) as f32
    };

    let mut main_scroll_children: Vec<UiComponentNode> = Vec::new();
    for entry_index in visible_indices
        .iter()
        .copied()
        .filter(|entry_index| {
            snapshot
                .entries
                .get(*entry_index)
                .map(AssetsCatalogEntry::is_directory)
                .unwrap_or(false)
        })
        .skip(window_start)
        .take(10)
    {
        let Some(entry) = snapshot.entries.get(entry_index) else {
            continue;
        };
        let mut card = UiComponentNode::action(
            format!("asset_browser.folder_card.{entry_index:03}"),
            entry.name.clone(),
            "asset_browser.folder.open",
        )
        .with_icon(ASSET_BROWSER_ICON_FOLDER)
        .with_value("Folder")
        .with_detail(entry.logical_path.clone())
        .with_tone(UiNodeTone::Accent)
        .tagged("folder-card");
        if hovered_entry_index == Some(entry_index) {
            card = card.tagged("hovered");
        }
        main_scroll_children.push(card);
    }

    for visible_idx in visible_indices
        .iter()
        .copied()
        .skip(window_start)
        .filter(|entry_index| {
            snapshot
                .entries
                .get(*entry_index)
                .map(|entry| !entry.is_directory())
                .unwrap_or(false)
        })
        .take(36)
    {
        let Some(entry) = snapshot.entries.get(visible_idx) else {
            continue;
        };
        let selected = visible_idx == selected_index;
        let hovered = hovered_entry_index == Some(visible_idx);
        let mut card = UiComponentNode::action(
            format!("asset_browser.asset_card.{visible_idx:03}"),
            entry.name.clone(),
            "asset_browser.asset.select",
        )
        .with_icon(icon_for_entry(entry))
        .with_value(asset_type_label(entry))
        .with_detail(format!("{} · {}", entry.import_stage, entry.import_action))
        .with_tone(if selected {
            UiNodeTone::Accent
        } else {
            UiNodeTone::Normal
        })
        .tagged("asset-card")
        .tagged(entry.kind.clone())
        .tagged(match view_mode {
            CatalogViewMode::List => "list-row",
            CatalogViewMode::Tree => "tree-row",
            _ => "grid-card",
        });
        if selected {
            card = card.tagged("selected");
        }
        if hovered {
            card = card.tagged("hovered");
        }
        main_scroll_children.push(card);
    }

    let mut main_scroll = UiComponentNode::row(
        "asset_browser.main_scroll",
        format!("{} visible entries", visible_indices.len()),
    )
    .with_detail("generic ScrollContainer: wheel/drag goes through ui.dispatch_input_v1")
    .with_tone(UiNodeTone::Normal)
    .with_prop("overflow", serde_json::json!("auto"))
    .with_prop("h_px", serde_json::json!(154.0))
    .with_prop(
        "row_h_px",
        serde_json::json!(if view_mode == CatalogViewMode::Grid {
            34.0
        } else {
            26.0
        }),
    )
    .with_prop("scrollbar_w_px", serde_json::json!(8.0))
    .with_prop("scroll_offset_01", serde_json::json!(scroll_offset_01))
    .with_prop("scroll_page_01", serde_json::json!(scroll_page_01))
    .with_prop(
        "scrollbar_always",
        serde_json::json!(visible_indices.len() > window_size),
    )
    .tagged("scroll-container")
    .tagged("asset-browser-main");
    main_scroll.component_id = match view_mode {
        CatalogViewMode::Grid => UI_COMPONENT_GRID,
        CatalogViewMode::Tree => UI_COMPONENT_TREE,
        CatalogViewMode::List | CatalogViewMode::Inspector => UI_COMPONENT_LIST,
    }
    .to_owned();
    main_scroll
        .props
        .insert("item_w_px".to_owned(), serde_json::json!(132.0));
    main_scroll
        .props
        .insert("item_h_px".to_owned(), serde_json::json!(88.0));
    main_scroll
        .props
        .insert("draw_panel".to_owned(), serde_json::json!(true));
    main_scroll.children = main_scroll_children;
    components.push(main_scroll);

    push_asset_grid_details_and_status_components(
        &mut components,
        snapshot,
        selected_entry,
        focus_scope,
        document_actions,
        context_menu_open,
        last_action_result,
        folder_count,
        asset_count,
    );

    apply_catalog_component_layout(&mut components, &geometry);

    build_assets_catalog_surface_node(
        frame_index,
        snapshot,
        selected_index,
        view_mode,
        search_query,
        focus_scope,
        hovered_entry_index,
        document_actions,
        context_menu_open,
        last_action_result,
        folder_count,
        asset_count,
        window_start,
        scroll_offset_01,
        scroll_page_01,
        body_lines,
        components,
    )
}
