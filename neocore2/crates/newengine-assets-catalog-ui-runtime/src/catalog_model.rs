use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CatalogFocusScope {
    Tree,
    Breadcrumb,
    Search,
    Grid,
    Inspector,
}

impl CatalogFocusScope {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Tree => "tree",
            Self::Breadcrumb => "breadcrumb",
            Self::Search => "search",
            Self::Grid => "grid",
            Self::Inspector => "inspector",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct CatalogWorkspaceGeometry {
    pub(crate) panel_x: f32,
    pub(crate) panel_y: f32,
    pub(crate) panel_w: f32,
    pub(crate) panel_h: f32,
    pub(crate) sidebar_x: f32,
    pub(crate) sidebar_w: f32,
    pub(crate) main_x: f32,
    pub(crate) main_w: f32,
    pub(crate) details_x: f32,
    pub(crate) details_w: f32,
    pub(crate) content_top: f32,
    pub(crate) content_h: f32,
    pub(crate) tab_h: f32,
    pub(crate) toolbar_h: f32,
}

pub(crate) fn catalog_workspace_geometry(surface_size_px: [u32; 2]) -> CatalogWorkspaceGeometry {
    let style = assets_catalog_surface_style();
    let style_tags = vec![
        "asset-catalog".to_owned(),
        "docked-panel".to_owned(),
        "dock-bottom".to_owned(),
        "engine-ui-node".to_owned(),
    ];
    let layout = ui_surface_node_layout(surface_size_px, &style_tags, &style, 5, 2);
    let panel_x = layout.panel_x;
    let panel_y = layout.panel_y;
    let panel_w = layout.panel_w;
    let panel_h = layout.panel_h;
    let tab_h = 34.0;
    let toolbar_h = 40.0;
    let breadcrumb_h = 34.0;
    let inner_gap = 8.0;
    let content_top = panel_y + tab_h + toolbar_h + breadcrumb_h + inner_gap;
    let content_bottom = panel_y + panel_h - 30.0;
    let content_h = (content_bottom - content_top).max(96.0);
    let sidebar_w = (panel_w * 0.18).clamp(210.0, 286.0);
    let details_w = (panel_w * 0.20).clamp(240.0, 330.0);
    let sidebar_x = panel_x + inner_gap;
    let details_x = panel_x + panel_w - details_w - inner_gap;
    let main_x = sidebar_x + sidebar_w + inner_gap;
    let main_w = (details_x - main_x - inner_gap).max(320.0);
    CatalogWorkspaceGeometry {
        panel_x,
        panel_y,
        panel_w,
        panel_h,
        sidebar_x,
        sidebar_w,
        main_x,
        main_w,
        details_x,
        details_w,
        content_top,
        content_h,
        tab_h,
        toolbar_h,
    }
}

pub(crate) fn push_catalog_regions(
    components: &mut Vec<UiComponentNode>,
    geometry: &CatalogWorkspaceGeometry,
) {
    components.push(catalog_region(
        "asset_browser.region.tabs",
        geometry.panel_x,
        geometry.panel_y,
        geometry.panel_w,
        geometry.tab_h,
        [7, 12, 19, 252],
    ));
    components.push(catalog_region(
        "asset_browser.region.toolbar",
        geometry.panel_x,
        geometry.panel_y + geometry.tab_h,
        geometry.panel_w,
        geometry.toolbar_h,
        [9, 14, 22, 252],
    ));
    components.push(catalog_region(
        "asset_browser.region.breadcrumb",
        geometry.panel_x,
        geometry.panel_y + geometry.tab_h + geometry.toolbar_h,
        geometry.panel_w,
        34.0,
        [7, 11, 18, 252],
    ));
    components.push(catalog_region(
        "asset_browser.region.sidebar",
        geometry.sidebar_x,
        geometry.content_top,
        geometry.sidebar_w,
        geometry.content_h,
        [8, 13, 20, 248],
    ));
    components.push(catalog_region(
        "asset_browser.region.main",
        geometry.main_x,
        geometry.content_top,
        geometry.main_w,
        geometry.content_h,
        [5, 9, 15, 248],
    ));
    components.push(catalog_region(
        "asset_browser.region.details",
        geometry.details_x,
        geometry.content_top,
        geometry.details_w,
        geometry.content_h,
        [8, 13, 20, 248],
    ));
    components.push(catalog_region(
        "asset_browser.region.status",
        geometry.panel_x,
        geometry.panel_y + geometry.panel_h - 30.0,
        geometry.panel_w,
        30.0,
        [7, 11, 18, 252],
    ));
}

pub(crate) fn catalog_region(
    id: &str,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    fill: [u8; 4],
) -> UiComponentNode {
    let mut component = UiComponentNode::row(id, "")
        .tagged("region")
        .tagged("panel-region")
        .tagged("asset-browser-region");
    component.component_id = UI_COMPONENT_PANEL.to_owned();
    component
        .props
        .insert("interactive".to_owned(), json!(false));
    component.props.insert("draw_panel".to_owned(), json!(true));
    component.props.insert("fill_rgba".to_owned(), json!(fill));
    component
        .props
        .insert("border_rgba".to_owned(), json!([54, 70, 92, 150]));
    component.props.insert(
        "radius_px".to_owned(),
        json!(if h <= 40.0 { 0.0 } else { 7.0 }),
    );
    set_component_rect(&mut component, x, y, w, h);
    component
}

pub(crate) fn apply_catalog_component_layout(
    components: &mut [UiComponentNode],
    geometry: &CatalogWorkspaceGeometry,
) {
    let mut tab_x = geometry.panel_x + 14.0;
    let mut action_x = geometry.panel_x + 300.0;
    let mut sidebar_y = geometry.content_top;
    let mut details_y = geometry.content_top;
    let mut status_y = geometry.panel_y + geometry.panel_h - 34.0;
    let mut context_y = geometry.content_top + 42.0;

    for component in components.iter_mut() {
        let id = component.id.as_str();
        if id.starts_with("asset_browser.tab.") {
            set_component_rect(component, tab_x, geometry.panel_y + 6.0, 104.0, 26.0);
            tab_x += 112.0;
        } else if id == "asset_browser.toolbar" {
            set_component_rect(
                component,
                geometry.panel_x + 14.0,
                geometry.panel_y + geometry.tab_h + 6.0,
                270.0,
                28.0,
            );
        } else if id.starts_with("asset_browser.action.") {
            set_component_rect(
                component,
                action_x,
                geometry.panel_y + geometry.tab_h + 6.0,
                118.0,
                28.0,
            );
            action_x += 124.0;
        } else if id == "asset_browser.breadcrumb" {
            set_component_rect(
                component,
                geometry.panel_x + 14.0,
                geometry.panel_y + geometry.tab_h + geometry.toolbar_h + 6.0,
                geometry.panel_w * 0.55,
                28.0,
            );
        } else if id == "asset_browser.search" {
            let x = geometry.panel_x + geometry.panel_w * 0.62;
            set_component_rect(
                component,
                x,
                geometry.panel_y + geometry.tab_h + geometry.toolbar_h + 6.0,
                (geometry.panel_x + geometry.panel_w - x - 14.0).max(180.0),
                28.0,
            );
        } else if id.starts_with("asset_browser.sidebar.") {
            set_component_rect(
                component,
                geometry.sidebar_x,
                sidebar_y,
                geometry.sidebar_w,
                24.0,
            );
            sidebar_y += 26.0;
        } else if id == "asset_browser.main_scroll" {
            component
                .props
                .insert("h_px".to_owned(), json!(geometry.content_h));
            component
                .props
                .insert("w_px".to_owned(), json!(geometry.main_w));
            set_component_rect(
                component,
                geometry.main_x,
                geometry.content_top,
                geometry.main_w,
                geometry.content_h,
            );
        } else if id.starts_with("asset_browser.details.") || id == "asset_browser.selection.bridge"
        {
            set_component_rect(
                component,
                geometry.details_x,
                details_y,
                geometry.details_w,
                24.0,
            );
            details_y += 27.0;
        } else if id.starts_with("asset_browser.context_menu") {
            let w = 240.0_f32.min(geometry.main_w.max(160.0));
            set_component_rect(
                component,
                (geometry.details_x - w - 10.0).max(geometry.main_x),
                context_y,
                w,
                28.0,
            );
            context_y += 31.0;
        } else if id == "asset_browser.action_result"
            || id == "asset_browser.status"
            || id.starts_with("asset_browser.warning.")
        {
            set_component_rect(
                component,
                geometry.panel_x + 14.0,
                status_y,
                geometry.panel_w - 28.0,
                24.0,
            );
            status_y += 26.0;
        }
    }

    components.sort_by(|a, b| {
        let ay = component_rect_number(a, "y_px").unwrap_or(f32::MAX);
        let by = component_rect_number(b, "y_px").unwrap_or(f32::MAX);
        let ax = component_rect_number(a, "x_px").unwrap_or(f32::MAX);
        let bx = component_rect_number(b, "x_px").unwrap_or(f32::MAX);
        ay.partial_cmp(&by)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| ax.partial_cmp(&bx).unwrap_or(std::cmp::Ordering::Equal))
            .then_with(|| component_paint_rank(a).cmp(&component_paint_rank(b)))
            .then_with(|| a.id.cmp(&b.id))
    });
}

pub(crate) fn component_paint_rank(component: &UiComponentNode) -> u8 {
    if component
        .state_tags
        .iter()
        .any(|tag| tag == "region" || tag == "panel-region")
    {
        0
    } else {
        1
    }
}

pub(crate) fn set_component_rect(component: &mut UiComponentNode, x: f32, y: f32, w: f32, h: f32) {
    component.props.insert("x_px".to_owned(), json!(x.max(0.0)));
    component.props.insert("y_px".to_owned(), json!(y.max(0.0)));
    component.props.insert("w_px".to_owned(), json!(w.max(1.0)));
    component.props.insert("h_px".to_owned(), json!(h.max(1.0)));
}

pub(crate) fn component_rect_number(component: &UiComponentNode, key: &str) -> Option<f32> {
    component
        .props
        .get(key)
        .and_then(|value| value.as_f64())
        .map(|value| value as f32)
}

pub(crate) fn hit_breadcrumb_path(
    snapshot: &AssetsCatalogSnapshot,
    mx: f32,
    start_x: f32,
    max_w: f32,
) -> String {
    let normalized = normalize_catalog_path(&snapshot.logical_path);
    if normalized.is_empty() {
        return String::new();
    }
    let mut x = start_x;
    let mut path = String::new();
    for segment in normalized.split('/') {
        if !path.is_empty() {
            path.push('/');
        }
        path.push_str(segment);
        let w = (segment.chars().count() as f32 * 8.0 + 24.0).clamp(34.0, 160.0);
        if mx >= x && mx <= x + w && x - start_x < max_w {
            return path.clone();
        }
        x += w + 6.0;
    }
    normalized
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CatalogViewMode {
    Tree,
    List,
    Grid,
    Inspector,
}

impl CatalogViewMode {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Tree => "tree",
            Self::List => "list",
            Self::Grid => "grid",
            Self::Inspector => "inspector",
        }
    }

    pub(crate) fn previous(self) -> Self {
        match self {
            Self::Tree => Self::Inspector,
            Self::List => Self::Tree,
            Self::Grid => Self::List,
            Self::Inspector => Self::Grid,
        }
    }

    pub(crate) fn next(self) -> Self {
        match self {
            Self::Tree => Self::List,
            Self::List => Self::Grid,
            Self::Grid => Self::Inspector,
            Self::Inspector => Self::Tree,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct AssetsCatalogSnapshot {
    pub(crate) logical_path: String,
    pub(crate) entries: Vec<AssetsCatalogEntry>,
    pub(crate) sources: Vec<String>,
    pub(crate) formats: Vec<String>,
    pub(crate) warnings: Vec<String>,
    pub(crate) import_summary: String,
    pub(crate) import_queue_summary: String,
    pub(crate) package_writer_summary: String,
    pub(crate) route_diagnostics: String,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct AssetsCatalogEntry {
    pub(crate) name: String,
    pub(crate) kind: String,
    pub(crate) logical_path: String,
    pub(crate) extension: String,
    pub(crate) semantic_gateway: String,
    pub(crate) asset_kind: String,
    pub(crate) import_stage: String,
    pub(crate) import_action: String,
    pub(crate) dirty: bool,
    pub(crate) uid: String,
    pub(crate) thumbnail: String,
}

impl AssetsCatalogEntry {
    pub(crate) fn is_directory(&self) -> bool {
        let kind = self.kind.trim().to_ascii_lowercase();
        kind == "directory" || kind == "dir" || kind == "folder" || kind == "mount"
    }
}
