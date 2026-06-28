use super::*;

pub(crate) fn asset_grid_body_lines(
    snapshot: &AssetsCatalogSnapshot,
    folder_count: usize,
    asset_count: usize,
    visible_count: usize,
    focus_scope: CatalogFocusScope,
    search_query: &str,
) -> Vec<String> {
    let mut body_lines = Vec::new();
    body_lines.push(format!(
        "{} folders · {} assets · {} mounted sources · {} declared formats",
        folder_count,
        asset_count,
        snapshot.sources.len(),
        snapshot.formats.len(),
    ));
    body_lines.push(format!("Path: {}", display_path(&snapshot.logical_path)));
    body_lines
        .push("Content Browser panel · selection publisher · provider DTO consumer.".to_owned());
    body_lines.push(snapshot.import_summary.clone());
    body_lines.push(format!(
        "UI focus={} · query='{}' · visible={}",
        focus_scope.as_str(),
        search_query,
        visible_count
    ));
    body_lines
}

pub(crate) fn push_asset_grid_navigation_components(
    components: &mut Vec<UiComponentNode>,
    snapshot: &AssetsCatalogSnapshot,
    view_mode: CatalogViewMode,
    search_query: &str,
    focus_scope: CatalogFocusScope,
    document_actions: &[AssetDocumentAction],
    collapsed_paths: &BTreeSet<String>,
    hovered_entry_index: Option<usize>,
    visible_indices: &[usize],
) {
    for (id, label, icon, mode, detail) in [
        (
            "tree",
            "Tree",
            ASSET_BROWSER_ICON_FOLDER,
            CatalogViewMode::Tree,
            "hierarchy",
        ),
        (
            "list",
            "List",
            ASSET_BROWSER_ICON_GENERIC,
            CatalogViewMode::List,
            "dense rows",
        ),
        (
            "grid",
            "Grid",
            ASSET_BROWSER_ICON_TEXTURE,
            CatalogViewMode::Grid,
            "previews",
        ),
        (
            "inspector",
            "Inspector",
            ASSET_BROWSER_ICON_GENERIC,
            CatalogViewMode::Inspector,
            "schema DTO · providers",
        ),
    ] {
        let tab = UiComponentNode::action(
            format!("asset_browser.tab.{id}"),
            label,
            format!("asset_browser.view.{id}"),
        )
        .with_icon(icon)
        .with_detail(detail)
        .with_tone(if view_mode == mode {
            UiNodeTone::Accent
        } else {
            UiNodeTone::Normal
        })
        .tagged("tab")
        .tagged(if view_mode == mode {
            "active"
        } else {
            "inactive"
        });
        components.push(tab);
    }
    let toolbar_labels = catalog_toolbar_items(document_actions)
        .into_iter()
        .map(|item| match item {
            CatalogToolbarItem::DocumentAction { label, enabled, .. } => {
                if enabled {
                    label
                } else {
                    format!("{} · disabled", label)
                }
            }
            CatalogToolbarItem::ViewAction { label, .. } => label.to_owned(),
        })
        .collect::<Vec<_>>()
        .join("    ");
    components.push(
        UiComponentNode::row("asset_browser.toolbar", toolbar_labels)
            .with_detail("Document actions are provider-declared AssetPatch templates; view actions stay local UI state")
            .with_tone(UiNodeTone::Normal)
            .tagged("toolbar")
            .tagged("asset-patch-dispatcher"),
    );
    for action in document_actions.iter() {
        let action_row = UiComponentNode::action(
            format!("asset_browser.action.{}", component_id_fragment(&action.id)),
            action.label.clone(),
            action.id.clone(),
        )
        .with_detail(if action.enabled {
            action.tooltip.clone()
        } else {
            action.disabled_reason.clone()
        })
        .with_tone(if action.enabled {
            UiNodeTone::Normal
        } else {
            UiNodeTone::Disabled
        })
        .tagged("action")
        .tagged("asset-patch")
        .tagged("toolbar")
        .tagged(if action.enabled {
            "enabled"
        } else {
            "disabled"
        });
        components.push(action_row);
    }

    let mut breadcrumb = UiComponentNode::row(
        "asset_browser.breadcrumb",
        format!("Content  /  {}", display_path(&snapshot.logical_path)),
    )
    .with_detail("engine.assets.vfs_list_json_v1")
    .with_tone(UiNodeTone::Accent)
    .tagged("breadcrumb");
    breadcrumb.action_id = Some("asset_browser.breadcrumb.open".to_owned());
    components.push(breadcrumb);
    let mut search = UiComponentNode::action(
        "asset_browser.search",
        "Search Content",
        "asset_browser.search.focus",
    )
    .with_value(if search_query.is_empty() {
        format!("Search {}...", browser_folder_label(&snapshot.logical_path))
    } else {
        search_query.to_owned()
    })
    .with_detail("Search/filter is local UI state; backend remains engine.assets")
    .with_tone(if focus_scope == CatalogFocusScope::Search {
        UiNodeTone::Accent
    } else {
        UiNodeTone::Normal
    })
    .tagged("search")
    .tagged(if focus_scope == CatalogFocusScope::Search {
        "focused"
    } else {
        "idle"
    });
    search.component_id = UI_COMPONENT_INPUT.to_owned();
    components.push(search);

    components.push(
        UiComponentNode::row("asset_browser.sidebar.favorites", "Favorites")
            .with_tone(UiNodeTone::Normal)
            .tagged("sidebar"),
    );
    components.push({
        UiComponentNode::action(
            "asset_browser.sidebar.root",
            "All Content",
            "asset_browser.root.open",
        )
        .with_icon(ASSET_BROWSER_ICON_FOLDER)
        .with_detail("root")
        .with_tone(if snapshot.logical_path.is_empty() {
            UiNodeTone::Accent
        } else {
            UiNodeTone::Normal
        })
        .tagged("sidebar")
        .tagged("folder")
    });
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
        .take(18)
    {
        let Some(entry) = snapshot.entries.get(entry_index) else {
            continue;
        };
        let depth = entry
            .logical_path
            .split('/')
            .count()
            .saturating_sub(1)
            .min(3);
        let collapsed = collapsed_paths.contains(&normalize_catalog_path(&entry.logical_path));
        let label = format!(
            "{}{} {}",
            "  ".repeat(depth),
            if collapsed { "▸" } else { "▾" },
            entry.name
        );
        let mut row = UiComponentNode::action(
            format!("asset_browser.sidebar.folder.{entry_index:03}"),
            label,
            "asset_browser.folder.open",
        )
        .with_icon(ASSET_BROWSER_ICON_FOLDER)
        .with_detail(display_path(&entry.logical_path))
        .with_tone(if snapshot.logical_path == entry.logical_path {
            UiNodeTone::Accent
        } else {
            UiNodeTone::Normal
        })
        .tagged("sidebar")
        .tagged("folder");
        if hovered_entry_index == Some(entry_index) {
            row = row.tagged("hovered");
        }
        components.push(row);
    }
}

pub(crate) fn push_asset_grid_details_and_status_components(
    components: &mut Vec<UiComponentNode>,
    snapshot: &AssetsCatalogSnapshot,
    selected_entry: Option<&AssetsCatalogEntry>,
    focus_scope: CatalogFocusScope,
    document_actions: &[AssetDocumentAction],
    context_menu_open: bool,
    last_action_result: Option<&AssetPatchResult>,
    folder_count: usize,
    asset_count: usize,
) {
    if let Some(entry) = selected_entry {
        components.push({
            let mut details =
                UiComponentNode::row("asset_browser.details.title", entry.name.clone())
                    .with_icon(icon_for_entry(entry))
                    .with_value(asset_type_label(entry))
                    .with_tone(UiNodeTone::Accent)
                    .tagged("details")
                    .tagged("details-title");
            details.action_id = Some("asset_browser.details.inspect".to_owned());
            details
        });
        for (id, label, value) in [
            ("path", "Path", display_path(&entry.logical_path)),
            ("type", "Type", asset_type_label(entry)),
            (
                "extension",
                "Extension",
                if entry.extension.is_empty() {
                    "directory".to_owned()
                } else {
                    entry.extension.clone()
                },
            ),
            ("gateway", "Gateway", entry.semantic_gateway.clone()),
            (
                "uid",
                "UID",
                if entry.uid.is_empty() {
                    "pending".to_owned()
                } else {
                    entry.uid.clone()
                },
            ),
            (
                "import",
                "Import",
                format!("{} / {}", entry.import_stage, entry.import_action),
            ),
            (
                "thumbnail",
                "Preview",
                if entry.thumbnail.is_empty() {
                    preview_plan_label(entry).to_owned()
                } else {
                    entry.thumbnail.clone()
                },
            ),
            ("providers", "Providers", snapshot.route_diagnostics.clone()),
            (
                "package_writer",
                "Package Writer",
                snapshot.package_writer_summary.clone(),
            ),
            (
                "ownership",
                "UI Role",
                "selection publisher; no local right inspector".to_owned(),
            ),
            (
                "focus",
                "Focus Graph",
                format!("scope={} modal=false z=970", focus_scope.as_str()),
            ),
        ] {
            components.push(
                UiComponentNode::row(format!("asset_browser.details.{id}"), label)
                    .with_value(value)
                    .with_tone(UiNodeTone::Normal)
                    .tagged("details"),
            );
        }
        components.push(
            UiComponentNode::row("asset_browser.selection.bridge", "Published Selection")
                .with_value(entry.logical_path.clone())
                .with_detail("global Right Edit Window consumes EditorSelectionContext and calls engine.assets.inspect")
                .with_tone(UiNodeTone::Accent)
                .tagged("details")
                .tagged("selection-context"),
        );
        if context_menu_open {
            components.push(
                UiComponentNode::row("asset_browser.context_menu.title", "Asset Actions")
                    .with_detail("provider-declared actions; dispatch emits AssetPatch DTO through engine.assets.edit")
                    .with_tone(UiNodeTone::Accent)
                    .tagged("context-menu"),
            );
            for action in document_actions.iter() {
                let mut row = UiComponentNode::row(
                    format!(
                        "asset_browser.context_menu.{}",
                        component_id_fragment(&action.id)
                    ),
                    action.label.clone(),
                )
                .with_detail(if action.enabled {
                    action.tooltip.clone()
                } else {
                    action.disabled_reason.clone()
                })
                .with_tone(if action.enabled {
                    UiNodeTone::Normal
                } else {
                    UiNodeTone::Disabled
                })
                .tagged("context-menu")
                .tagged("asset-patch");
                row.action_id = Some(action.id.clone());
                components.push(row);
            }
        }
    }

    if let Some(result) = last_action_result {
        let diagnostic = result
            .diagnostics
            .last()
            .map(|diag| diag.message.clone())
            .unwrap_or_else(|| "Asset action completed without diagnostics".to_owned());
        components.push(
            UiComponentNode::row(
                "asset_browser.action_result",
                if result.written {
                    "Asset write complete"
                } else if result.accepted {
                    "Asset patch accepted"
                } else {
                    "Asset action blocked"
                },
            )
            .with_detail(diagnostic)
            .with_tone(if result.accepted {
                UiNodeTone::Accent
            } else {
                UiNodeTone::Danger
            })
            .tagged("status")
            .tagged("asset-patch-result"),
        );
    }

    components.push(
        UiComponentNode::row(
            "asset_browser.status",
            format!("Showing {} of {} assets", asset_count.min(36), asset_count),
        )
        .with_detail(format!(
            "{} folders · {} · {} · F1 close · arrows navigate",
            folder_count, snapshot.import_queue_summary, snapshot.package_writer_summary
        ))
        .with_tone(UiNodeTone::Accent)
        .tagged("status"),
    );
    for (idx, warning) in snapshot.warnings.iter().take(4).enumerate() {
        components.push(
            UiComponentNode::row(format!("asset_browser.warning.{idx}"), warning.clone())
                .with_icon(ASSET_BROWSER_ICON_GENERIC)
                .with_tone(UiNodeTone::Danger)
                .tagged("status")
                .tagged("warning"),
        );
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn build_assets_catalog_surface_node(
    frame_index: u64,
    snapshot: &AssetsCatalogSnapshot,
    selected_index: usize,
    view_mode: CatalogViewMode,
    search_query: &str,
    focus_scope: CatalogFocusScope,
    hovered_entry_index: Option<usize>,
    document_actions: &[AssetDocumentAction],
    context_menu_open: bool,
    last_action_result: Option<&AssetPatchResult>,
    folder_count: usize,
    asset_count: usize,
    window_start: usize,
    scroll_offset_01: f32,
    scroll_page_01: f32,
    body_lines: Vec<String>,
    components: Vec<UiComponentNode>,
) -> UiSurfaceNode {
    let mut node = UiSurfaceNode::new(ASSETS_CATALOG_SURFACE_ID, ASSETS_CATALOG_UI_OWNER)
        .with_title("Content Browser")
        .with_subtitle("docked Content Browser panel over engine.assets")
        .with_body_lines(body_lines)
        .with_footer_lines(vec![
            "Docked panel · mouse hover/click/wheel · type to search · arrows/gamepad navigate · Enter Open/Inspect".to_owned(),
            "Content Browser publishes selection; global Right Edit Window renders AssetDocument DTOs".to_owned(),
        ])
        .with_theme(ASSETS_CATALOG_THEME_ID)
        .with_style_ref(UI_THEME_ASSET_NORTHSTAR_EDITOR)
        .with_style(assets_catalog_surface_style())
        .with_component(UI_COMPONENT_PANEL)
        .with_components(components)
        .with_metric("frame_index", serde_json::json!(frame_index))
        .with_metric("current_path", serde_json::json!(snapshot.logical_path.as_str()))
        .with_metric("selected_index", serde_json::json!(selected_index))
        .with_metric("view_mode", serde_json::json!(view_mode.as_str()))
        .with_metric("scroll_window_start", serde_json::json!(window_start))
        .with_metric("scroll_offset_01", serde_json::json!(scroll_offset_01))
        .with_metric("scroll_page_01", serde_json::json!(scroll_page_01))
        .with_metric("search_query", serde_json::json!(search_query))
        .with_metric("focus_scope", serde_json::json!(focus_scope.as_str()))
        .with_metric("modal_stack", serde_json::json!([ASSETS_CATALOG_SURFACE_ID]))
        .with_metric("hovered_entry_index", serde_json::json!(hovered_entry_index))
        .with_metric("import_summary", serde_json::json!(snapshot.import_summary.as_str()))
        .with_metric("package_writer", serde_json::json!(snapshot.package_writer_summary.as_str()))
        .with_metric("folder_count", serde_json::json!(folder_count))
        .with_metric("asset_count", serde_json::json!(asset_count))
        .with_metric("source_count", serde_json::json!(snapshot.sources.len()))
        .with_metric("format_count", serde_json::json!(snapshot.formats.len()))
        .with_metric("document_action_count", serde_json::json!(document_actions.len()))
        .with_metric("context_menu_open", serde_json::json!(context_menu_open))
        .with_metric("last_action_written", serde_json::json!(last_action_result.map(|result| result.written)));
    node.modal = false;
    node.z_order = 220;
    node.style_tags = vec![
        "workspace".to_owned(),
        "explorer-grid".to_owned(),
        "asset-catalog".to_owned(),
        "docked-panel".to_owned(),
        "dock-bottom".to_owned(),
        "engine-ui-node".to_owned(),
        "noir-editor".to_owned(),
    ];
    node
}
