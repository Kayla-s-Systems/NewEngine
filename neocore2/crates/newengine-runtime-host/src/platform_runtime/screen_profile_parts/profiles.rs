use super::*;

impl Default for EditorScreen {
    fn default() -> Self {
        Self {
            descriptor: editor_screen_descriptor(),
        }
    }
}

impl EditorScreen {
    pub(super) fn surface_node(&self, frame_index: u64, runtime_mode: UiEditorRuntimeMode, layout: &EditorLayoutMetrics, active_menu_id: Option<&str>) -> UiSurfaceNode {
        editor_screen_surface_node(&self.descriptor, frame_index, runtime_mode, layout, active_menu_id)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct GameScreen {
    descriptor: UiScreenProfileDescriptor,
}

impl Default for GameScreen {
    fn default() -> Self {
        Self {
            descriptor: game_screen_descriptor(None),
        }
    }
}

impl GameScreen {
    pub(super) fn with_game_ui_root(surface_id: impl Into<String>) -> Self {
        Self {
            descriptor: game_screen_descriptor(Some(surface_id.into())),
        }
    }

    pub(super) fn surface_node(&self, frame_index: u64) -> Option<UiSurfaceNode> {
        game_screen_surface_node(&self.descriptor, frame_index)
    }
}

pub(super) fn screen_profile_descriptor(profile: UiScreenProfile, game_ui_root: Option<String>) -> UiScreenProfileDescriptor {
    match profile {
        UiScreenProfile::Editor => editor_screen_descriptor(),
        UiScreenProfile::Game => game_screen_descriptor(game_ui_root),
        UiScreenProfile::Headless => headless_screen_descriptor(),
    }
}

pub(super) fn editor_screen_descriptor() -> UiScreenProfileDescriptor {
    UiScreenProfileDescriptor {
        version: 1,
        profile: UiScreenProfile::Editor,
        layout_id: EDITOR_LAYOUT_ID.to_owned(),
        surface_id: UI_SURFACE_EDITOR_SHELL.to_owned(),
        viewport_surface_id: DEFAULT_VIEWPORT_SURFACE.to_owned(),
        game_ui_root_surface_id: None,
        input_focus_policy: UiScreenInputFocusPolicy::EditorShell,
        panels: vec![
            screen_panel("top.toolbar", "Toolbar", "engine.ui.editor.toolbar", "engine.ui", "newengine.ui.editor.toolbar.dto.v1", true, false, ["top", "toolbar", "editor", "neui:assets/ui/editor/editor_shell.neui@toolbar"]),
            screen_panel("center.viewport", "Viewport", DEFAULT_VIEWPORT_SURFACE, "engine.render", "newengine.render.viewport.surface_ref.v1", true, false, ["center", "viewport", "render-target"]),
            screen_panel("center.viewport_gizmos", "Viewport Gizmos", "engine.ui.editor.viewport_gizmos", "engine.ui", "newengine.ui.editor.viewport_gizmos.node_tree.v1", true, false, ["center", "viewport-gizmos", "gizmo", "neui:assets/ui/editor/viewport_gizmos.neui@surface"]),
            screen_panel("left.scene_tree", "Scene Tree", "engine.ui.editor.scene_tree", "engine.scene", "newengine.scene.tree_snapshot.schema_driven.v1", true, false, ["left", "scene-tree", "scene", "world", "neui:assets/ui/editor/scene_tree.neui@surface"]),
            screen_panel("right.inspector", "Inspector", "engine.ui.editor.inspector", "engine.schema", "newengine.schema.properties.inspector.v1", true, false, ["right", "inspector", "schema", "properties", "neui:assets/ui/editor/inspector.neui@surface"]),
            screen_panel("bottom.asset_browser", "Asset Browser", "ui.assets.catalog", "engine.assets", "newengine.assets.catalog_ui.asset_document_dto.v1", true, false, ["bottom", "assets", "asset-browser", "content-browser", "neui:assets/ui/editor/content_browser.neui@editor.asset_browser"]),
            screen_panel("bottom.import_queue", "Import Queue", "engine.ui.editor.import_queue", "engine.assets", "newengine.assets.import_queue.snapshot.v1", true, false, ["bottom", "import-queue", "assets", "jobs", "neui:assets/ui/editor/import_queue.neui@surface"]),
            screen_panel("bottom.output_log", "Output Log", "engine.ui.editor.output_log", "engine.diagnostics", "newengine.diagnostics.output_log_snapshot.v1", true, false, ["bottom", "output-log", "console", "diagnostics", "neui:assets/ui/editor/output_log.neui@surface"]),
            screen_panel("bottom.profiler_diagnostics", "Profiler / Diagnostics", "engine.ui.editor.profiler_diagnostics", "engine.diagnostics", "newengine.diagnostics.profiler_route_snapshot.v1", true, false, ["bottom", "profiler", "diagnostics", "gateway", "jobs", "neui:assets/ui/editor/profiler_diagnostics.neui@surface"]),
        ],
        diagnostics: vec![
            "EditorScreen is a UI composition profile, not a backend domain.".to_owned(),
            "Panels must consume DTOs/snapshots and opaque handles only.".to_owned(),
            "Render backend selection is untouched by screen profile selection.".to_owned(),
        ],
    }
}

pub(super) fn game_screen_descriptor(game_ui_root: Option<String>) -> UiScreenProfileDescriptor {
    let mut panels = Vec::new();
    if let Some(root) = game_ui_root.as_ref().filter(|it| !it.trim().is_empty()) {
        panels.push(screen_panel(
            "game.ui_root",
            "Game UI Root",
            root,
            "engine.ui",
            "newengine.ui.surface_node.game_owned.v1",
            false,
            false,
            ["game-ui", "game-owned", "runtime-presentation"],
        ));
    }

    UiScreenProfileDescriptor {
        version: 1,
        profile: UiScreenProfile::Game,
        layout_id: GAME_LAYOUT_ID.to_owned(),
        surface_id: UI_SURFACE_GAME_PRESENTATION.to_owned(),
        viewport_surface_id: DEFAULT_VIEWPORT_SURFACE.to_owned(),
        game_ui_root_surface_id: game_ui_root,
        input_focus_policy: UiScreenInputFocusPolicy::GameViewport,
        panels,
        diagnostics: vec![
            "GameScreen is clean runtime presentation.".to_owned(),
            "Editor toolbar/outliner/right edit window/content browser are absent unless explicitly published as debug overlays.".to_owned(),
        ],
    }
}

pub(super) fn headless_screen_descriptor() -> UiScreenProfileDescriptor {
    UiScreenProfileDescriptor {
        version: 1,
        profile: UiScreenProfile::Headless,
        layout_id: "engine.ui.screen.headless.v1".to_owned(),
        surface_id: UI_SURFACE_SCREEN_ROOT.to_owned(),
        viewport_surface_id: String::new(),
        game_ui_root_surface_id: None,
        input_focus_policy: UiScreenInputFocusPolicy::Headless,
        panels: Vec::new(),
        diagnostics: vec!["Headless profile publishes no visual screen shell.".to_owned()],
    }
}

pub(super) fn editor_screen_surface_node(descriptor: &UiScreenProfileDescriptor, frame_index: u64, runtime_mode: UiEditorRuntimeMode, layout: &EditorLayoutMetrics, active_menu_id: Option<&str>) -> UiSurfaceNode {
    let body_lines = Vec::new();
    let footer_lines = vec![format!("mode={} · 1 Edit · 2 Simulate · 3 Play", runtime_mode.id())];
    let mut metrics = screen_metrics(descriptor, frame_index);
    metrics.insert("editor_panel_count".to_owned(), serde_json::json!(descriptor.panels.len()));
    metrics.insert("menu_h".to_owned(), serde_json::json!(layout.menu_h));
    metrics.insert("toolbar_h".to_owned(), serde_json::json!(layout.toolbar_h));
    metrics.insert("status_h".to_owned(), serde_json::json!(layout.status_h));
    metrics.insert("bottom_h".to_owned(), serde_json::json!(layout.bottom_h));
    metrics.insert("left_w".to_owned(), serde_json::json!(layout.left_w));
    metrics.insert("right_w".to_owned(), serde_json::json!(layout.right_w));
    metrics.insert("gap".to_owned(), serde_json::json!(layout.gap));
    metrics.insert("bottom_y".to_owned(), serde_json::json!(layout.bottom_y));
    metrics.insert("viewport_x".to_owned(), serde_json::json!(layout.viewport_x));
    metrics.insert("viewport_y".to_owned(), serde_json::json!(layout.viewport_y));
    metrics.insert("viewport_w".to_owned(), serde_json::json!(layout.viewport_w));
    metrics.insert("viewport_h".to_owned(), serde_json::json!(layout.viewport_h));
    metrics.insert("dock_left_visible".to_owned(), serde_json::json!(layout.left_visible));
    metrics.insert("dock_right_visible".to_owned(), serde_json::json!(layout.right_visible));
    metrics.insert("dock_bottom_visible".to_owned(), serde_json::json!(layout.bottom_visible));
    metrics.insert("source_ref".to_owned(), serde_json::json!(EDITOR_SHELL_NEUI_REF));
    metrics.insert("ui_request_transport".to_owned(), serde_json::json!("UiNodeTreeRequest"));

    UiSurfaceNode {
        version: 1,
        surface_id: descriptor.surface_id.clone(),
        source: SCREEN_PROFILE_SOURCE.to_owned(),
        visible: true,
        modal: false,
        z_order: 100,
        title: EDITOR_CHROME.product_title.to_owned(),
        subtitle: "Editor shell · viewport contained inside UI · simulation stopped until command".to_owned(),
        body_lines,
        footer_lines,
        style_tags: vec![
            "retained".to_owned(),
            "screen-profile".to_owned(),
            "editor-screen".to_owned(),
            "editor-shell".to_owned(),
            "fullscreen".to_owned(),
            "north-star".to_owned(),
        ],
        theme_id: UI_THEME_NORTHSTAR_EDITOR.to_owned(),
        style_ref: Some(UI_THEME_ASSET_NORTHSTAR_EDITOR.to_owned()),
        component_id: UI_COMPONENT_PANEL.to_owned(),
        components: editor_components(descriptor, runtime_mode, layout, active_menu_id),
        message: None,
        style: UiSurfaceStyle {
            anchor: UiSurfaceAnchor::TopLeft,
            min_size_px: [900.0, 520.0],
            max_size_px: [4096.0, 4096.0],
            margin_px: [0.0, 0.0],
            padding_px: [10.0, 28.0, 10.0, 18.0],
            row_pitch_px: 15.0,
            panel_rgba: [7, 10, 15, 255],
            panel_header_rgba: [12, 17, 25, 250],
            accent_rgba: [73, 151, 255, 255],
            text_rgba: [225, 232, 242, 255],
            text_muted_rgba: [135, 149, 169, 255],
            danger_rgba: [238, 96, 86, 255],
            border_rgba: [66, 82, 107, 160],
            backdrop_rgba: [0, 0, 0, 0],
            shadow_alpha: 0,
            corner_radius_px: 7.0,
            border_px: 1.0,
            font: newengine_ui_api::UiFontStyle {
                stack: vec![UI_FONT_ASSET_EDITOR_SANS.to_owned(), UI_FONT_ASSET_EDITOR_DISPLAY.to_owned(), "Segoe UI".to_owned()],
                body_px: 11.0,
                title_px: 13.0,
                secondary_px: 9.5,
                line_height_px: 14.0,
                pixel_snap: false,
            },
            ..UiSurfaceStyle::default()
        },
        admission_policy: Default::default(),
        metrics,
    }
}

pub(super) fn game_screen_surface_node(descriptor: &UiScreenProfileDescriptor, frame_index: u64) -> Option<UiSurfaceNode> {
    let root = descriptor.game_ui_root_surface_id.as_ref()?.trim();
    if root.is_empty() {
        return None;
    }
    let diagnostic_panel_enabled = std::env::var("NEWENGINE_GAME_SCREEN_DIAGNOSTIC_PANEL")
        .map(|value| matches!(value.trim(), "1" | "true" | "TRUE" | "True" | "on" | "ON" | "On"))
        .unwrap_or(false);
    let mut metrics = screen_metrics(descriptor, frame_index);
    metrics.insert("game_ui_root_surface_id".to_owned(), serde_json::json!(root));
    if !diagnostic_panel_enabled {
        return Some(UiSurfaceNode {
            version: 1,
            surface_id: descriptor.surface_id.clone(),
            source: SCREEN_PROFILE_SOURCE.to_owned(),
            visible: true,
            modal: false,
            z_order: 520,
            title: String::new(),
            subtitle: String::new(),
            body_lines: vec!["+".to_owned()],
            footer_lines: Vec::new(),
            style_tags: vec!["retained".to_owned(), "hud".to_owned(), "crosshair".to_owned()],
            theme_id: UI_THEME_NORTHSTAR_EDITOR.to_owned(),
            style_ref: Some(UI_THEME_ASSET_NORTHSTAR_EDITOR.to_owned()),
            component_id: UI_COMPONENT_PANEL.to_owned(),
            components: Vec::new(),
            message: None,
            style: UiSurfaceStyle {
                anchor: UiSurfaceAnchor::Center,
                min_size_px: [28.0, 28.0],
                max_size_px: [44.0, 44.0],
                margin_px: [0.0, 0.0],
                padding_px: [0.0, 0.0, 0.0, 0.0],
                row_pitch_px: 16.0,
                panel_rgba: [0, 0, 0, 0],
                panel_header_rgba: [0, 0, 0, 0],
                backdrop_rgba: [0, 0, 0, 0],
                border_rgba: [255, 255, 255, 0],
                text_rgba: [235, 245, 255, 220],
                shadow_alpha: 0,
                ..UiSurfaceStyle::default()
            },
            admission_policy: Default::default(),
            metrics,
        });
    }
    Some(UiSurfaceNode {
        version: 1,
        surface_id: descriptor.surface_id.clone(),
        source: SCREEN_PROFILE_SOURCE.to_owned(),
        visible: true,
        modal: false,
        z_order: 480,
        title: "Game Screen".to_owned(),
        subtitle: "Clean runtime presentation; editor panels are not part of this profile".to_owned(),
        body_lines: vec![format!("Game UI root: {root}")],
        footer_lines: vec!["Editor shell is absent; debug overlays must be explicitly enabled.".to_owned()],
        style_tags: vec!["retained".to_owned(), "screen-profile".to_owned(), "game-screen".to_owned()],
        theme_id: UI_THEME_NORTHSTAR_EDITOR.to_owned(),
        style_ref: Some(UI_THEME_ASSET_NORTHSTAR_EDITOR.to_owned()),
        component_id: UI_COMPONENT_PANEL.to_owned(),
        components: descriptor.panels.first().map(|panel| panel_component(panel, true, false)).into_iter().collect(),
        message: None,
        style: UiSurfaceStyle {
            anchor: UiSurfaceAnchor::BottomRight,
            min_size_px: [300.0, 88.0],
            max_size_px: [520.0, 160.0],
            margin_px: [12.0, 12.0],
            padding_px: [18.0, 48.0, 18.0, 22.0],
            row_pitch_px: 22.0,
            ..UiSurfaceStyle::default()
        },
        admission_policy: Default::default(),
        metrics,
    })
}

