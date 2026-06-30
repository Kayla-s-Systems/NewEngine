#![forbid(unsafe_op_in_unsafe_fn)]

use std::collections::BTreeMap;

use newengine_ui_api::{
    UiComponentNode, UiNodeTone, UiScreenInputFocusPolicy, UiScreenPanelDescriptor,
    UiScreenProfile, UiScreenProfileDescriptor, UiSurfaceAnchor, UiSurfaceNode, UiSurfaceStyle,
    UI_COMPONENT_PANEL, UI_COMPONENT_ROW, UI_FONT_ASSET_EDITOR_SANS, UI_SURFACE_EDITOR_SHELL,
    UI_SURFACE_GAME_PRESENTATION, UI_SURFACE_SCREEN_ROOT, UI_THEME_ASSET_NORTHSTAR_EDITOR,
    UI_THEME_NORTHSTAR_EDITOR,
};

const SOURCE: &str = "engine.ui.screen_profile";
const EDITOR_LAYOUT_ID: &str = "engine.ui.screen.editor.v1";
const GAME_LAYOUT_ID: &str = "engine.ui.screen.game.v1";
const DEFAULT_VIEWPORT_SURFACE: &str = "engine.render.viewport.primary";

/// Editor screen composition profile.
///
/// This object owns only UI composition data. It does not own `engine.scene`,
/// `engine.world`, `engine.ecs`, `engine.entity`, `engine.assets` or render
/// backend state. Panels listed here are panel requests over DTO/snapshot
/// contracts and opaque handles.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorScreen {
    descriptor: UiScreenProfileDescriptor,
}

impl Default for EditorScreen {
    fn default() -> Self {
        Self {
            descriptor: editor_screen_descriptor(),
        }
    }
}

impl EditorScreen {
    #[inline]
    pub fn descriptor(&self) -> &UiScreenProfileDescriptor {
        &self.descriptor
    }

    #[inline]
    pub fn into_descriptor(self) -> UiScreenProfileDescriptor {
        self.descriptor
    }

    pub fn surface_node(&self, frame_index: u64) -> UiSurfaceNode {
        editor_screen_surface_node(&self.descriptor, frame_index)
    }
}

/// Game screen composition profile.
///
/// Game screen is deliberately clean: no toolbar, no outliner, no right edit window and
/// no editor diagnostics by default. The only optional visual child is a
/// game-authored UI root surface id supplied as data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameScreen {
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
    #[inline]
    pub fn with_game_ui_root(surface_id: impl Into<String>) -> Self {
        Self {
            descriptor: game_screen_descriptor(Some(surface_id.into())),
        }
    }

    #[inline]
    pub fn descriptor(&self) -> &UiScreenProfileDescriptor {
        &self.descriptor
    }

    #[inline]
    pub fn into_descriptor(self) -> UiScreenProfileDescriptor {
        self.descriptor
    }

    /// Returns a visible game-shell node only when a game-authored UI root was
    /// supplied. A plain Game Screen without game UI is intentionally no editor
    /// shell at all: render keeps presenting the viewport directly.
    pub fn surface_node(&self, frame_index: u64) -> Option<UiSurfaceNode> {
        game_screen_surface_node(&self.descriptor, frame_index)
    }
}

#[inline]
pub fn screen_profile_descriptor(
    profile: UiScreenProfile,
    game_ui_root: Option<String>,
) -> UiScreenProfileDescriptor {
    match profile {
        UiScreenProfile::Editor => editor_screen_descriptor(),
        UiScreenProfile::Game => game_screen_descriptor(game_ui_root),
        UiScreenProfile::Headless => headless_screen_descriptor(),
    }
}

pub fn editor_screen_descriptor() -> UiScreenProfileDescriptor {
    UiScreenProfileDescriptor {
        version: 1,
        profile: UiScreenProfile::Editor,
        layout_id: EDITOR_LAYOUT_ID.to_owned(),
        surface_id: UI_SURFACE_EDITOR_SHELL.to_owned(),
        viewport_surface_id: DEFAULT_VIEWPORT_SURFACE.to_owned(),
        game_ui_root_surface_id: None,
        input_focus_policy: UiScreenInputFocusPolicy::EditorShell,
        panels: vec![
            panel(
                "top.toolbar",
                "Toolbar",
                "engine.ui.editor.toolbar",
                "engine.ui",
                "newengine.ui.editor.toolbar.dto.v1",
                true,
                false,
                ["top", "toolbar", "editor"],
            ),
            panel(
                "center.viewport",
                "Viewport",
                DEFAULT_VIEWPORT_SURFACE,
                "engine.render",
                "newengine.render.viewport.surface_ref.v1",
                true,
                false,
                ["center", "viewport", "render-target"],
            ),
            panel(
                "left.outliner",
                "Scene / World Outliner",
                "engine.ui.editor.outliner",
                "engine.scene",
                "newengine.scene.world_outliner.readonly_snapshot.v1",
                true,
                false,
                ["left", "outliner", "scene", "world"],
            ),
            panel(
                "left.visibility",
                "ECS / Entity Visibility",
                "engine.ui.editor.entity_visibility",
                "engine.entity",
                "newengine.entity.visibility.opaque_handles.v1",
                false,
                false,
                ["left", "entity", "visibility", "opaque-handles"],
            ),
            panel(
                "right.edit_window",
                "Right Edit Window",
                "engine.ui.editor.edit_window",
                "engine.editor",
                "newengine.editor.selection_context.v1",
                true,
                false,
                [
                    "right",
                    "edit-window",
                    "selection-context",
                    "asset-document",
                    "component-dto",
                    "opaque-handles",
                ],
            ),
            panel(
                "right.world_settings",
                "World Settings",
                "engine.ui.editor.world_settings",
                "engine.world",
                "newengine.world.settings.dto.v1",
                false,
                false,
                ["right", "world-settings", "dto"],
            ),
            panel(
                "bottom.content_browser",
                "Content Browser",
                "ui.assets.catalog",
                "engine.assets",
                "newengine.assets.catalog_ui.asset_document_dto.v1",
                true,
                false,
                ["bottom", "assets", "content-browser", "ui-composition"],
            ),
            panel(
                "bottom.console",
                "Console / Output Log",
                "engine.ui.editor.console",
                "engine.console",
                "newengine.console.output_log_snapshot.v1",
                false,
                false,
                ["bottom", "console", "output-log"],
            ),
            panel(
                "bottom.shader_compile",
                "Shader Compile",
                "engine.ui.editor.shader_compile",
                "engine.jobs",
                "newengine.jobs.shader_compile_snapshot.v1",
                false,
                false,
                ["bottom", "shader", "jobs"],
            ),
            panel(
                "bottom.jobs_profiler",
                "Jobs / Profiler",
                "engine.ui.editor.profiler",
                "engine.jobs",
                "newengine.jobs.profiler_snapshot.v1",
                false,
                false,
                ["bottom", "jobs", "profiler"],
            ),
            panel(
                "bottom.gateway_diagnostics",
                "North Star Gateway",
                "engine.ui.editor.gateway_diagnostics",
                "engine.gateway_registry",
                "newengine.gateway.route_diagnostics.v1",
                true,
                false,
                ["bottom", "gateway", "diagnostics"],
            ),
        ],
        diagnostics: vec![
            "EditorScreen is a UI composition profile, not a backend domain.".to_owned(),
            "Panels must consume DTOs/snapshots and opaque handles only.".to_owned(),
            "Render backend selection is untouched by screen profile selection.".to_owned(),
        ],
    }
}

pub fn game_screen_descriptor(game_ui_root: Option<String>) -> UiScreenProfileDescriptor {
    let mut panels = Vec::new();
    if let Some(root) = game_ui_root.as_ref().filter(|it| !it.trim().is_empty()) {
        panels.push(panel(
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

pub fn headless_screen_descriptor() -> UiScreenProfileDescriptor {
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

fn editor_screen_surface_node(
    descriptor: &UiScreenProfileDescriptor,
    frame_index: u64,
) -> UiSurfaceNode {
    let body_lines = vec![
        "Editor profile publishes a compact dock shell; content is routed through DTO contracts."
            .to_owned(),
        "The viewport is a contained render target, not a fullscreen game background.".to_owned(),
    ];
    let footer_lines = vec![
        "Profile: Editor · simulation stopped until command · provider routes unchanged".to_owned(),
    ];
    let mut metrics = screen_metrics(descriptor, frame_index);
    metrics.insert(
        "editor_panel_count".to_owned(),
        serde_json::json!(descriptor.panels.len()),
    );

    UiSurfaceNode {
        version: 1,
        surface_id: descriptor.surface_id.clone(),
        source: SOURCE.to_owned(),
        visible: true,
        modal: false,
        z_order: 100,
        title: "North Star".to_owned(),
        subtitle: "Editor shell · viewport contained inside UI · simulation stopped until command"
            .to_owned(),
        body_lines,
        footer_lines,
        style_tags: vec![
            "retained".to_owned(),
            "screen-profile".to_owned(),
            "editor-screen".to_owned(),
            "fullscreen".to_owned(),
            "north-star".to_owned(),
        ],
        theme_id: UI_THEME_NORTHSTAR_EDITOR.to_owned(),
        style_ref: Some(UI_THEME_ASSET_NORTHSTAR_EDITOR.to_owned()),
        component_id: UI_COMPONENT_PANEL.to_owned(),
        components: editor_components(descriptor),
        message: None,
        style: UiSurfaceStyle {
            anchor: UiSurfaceAnchor::TopLeft,
            min_size_px: [900.0, 520.0],
            max_size_px: [4096.0, 4096.0],
            margin_px: [4.0, 4.0],
            padding_px: [12.0, 46.0, 12.0, 24.0],
            row_pitch_px: 18.0,
            font: newengine_ui_api::UiFontStyle {
                stack: vec![
                    UI_FONT_ASSET_EDITOR_SANS.to_owned(),
                    "Inter".to_owned(),
                    "Segoe UI".to_owned(),
                    "NotoSans".to_owned(),
                ],
                body_px: 11.0,
                title_px: 13.0,
                secondary_px: 9.5,
                line_height_px: 15.0,
                ..Default::default()
            },
            ..UiSurfaceStyle::default()
        },
        admission_policy: Default::default(),
        metrics,
    }
}

fn game_screen_surface_node(
    descriptor: &UiScreenProfileDescriptor,
    frame_index: u64,
) -> Option<UiSurfaceNode> {
    let root = descriptor.game_ui_root_surface_id.as_ref()?.trim();
    if root.is_empty() {
        return None;
    }
    let mut metrics = screen_metrics(descriptor, frame_index);
    metrics.insert(
        "game_ui_root_surface_id".to_owned(),
        serde_json::json!(root),
    );
    Some(UiSurfaceNode {
        version: 1,
        surface_id: descriptor.surface_id.clone(),
        source: SOURCE.to_owned(),
        visible: true,
        modal: false,
        z_order: 480,
        title: "Game Screen".to_owned(),
        subtitle: "Clean runtime presentation; editor panels are not part of this profile"
            .to_owned(),
        body_lines: vec![format!("Game UI root: {root}")],
        footer_lines: vec![
            "Editor shell is absent; debug overlays must be explicitly enabled.".to_owned(),
        ],
        style_tags: vec![
            "retained".to_owned(),
            "screen-profile".to_owned(),
            "game-screen".to_owned(),
        ],
        theme_id: UI_THEME_NORTHSTAR_EDITOR.to_owned(),
        style_ref: Some(UI_THEME_ASSET_NORTHSTAR_EDITOR.to_owned()),
        component_id: UI_COMPONENT_PANEL.to_owned(),
        components: vec![panel_component(&descriptor.panels[0])],
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

fn editor_components(descriptor: &UiScreenProfileDescriptor) -> Vec<UiComponentNode> {
    let mut out = Vec::new();
    out.push(
        UiComponentNode::row("editor.identity", "North Star")
            .with_value("Editor")
            .with_detail("editor profile · dock shell · DTO routed panels")
            .with_tone(UiNodeTone::Accent)
            .tagged("identity")
            .tagged("top")
            .with_tooltip("Editor shell root. Runtime providers stay behind engine.* gateways."),
    );
    out.extend(descriptor.panels.iter().map(panel_component));
    out
}

fn panel_component(panel: &UiScreenPanelDescriptor) -> UiComponentNode {
    let mut component = UiComponentNode {
        id: panel.slot_id.clone(),
        component_id: UI_COMPONENT_ROW.to_owned(),
        text: panel.label.clone(),
        value: Some(panel.source_gateway.clone()),
        detail: Some(panel.data_contract.clone()),
        icon: None,
        font_token: None,
        tone: if panel.required {
            UiNodeTone::Accent
        } else {
            UiNodeTone::Normal
        },
        state_tags: panel.tags.clone(),
        action_id: None,
        props: BTreeMap::new(),
        children: Vec::new(),
    };
    component
        .props
        .insert("surface_id".to_owned(), serde_json::json!(panel.surface_id));
    component
        .props
        .insert("required".to_owned(), serde_json::json!(panel.required));
    component
        .props
        .insert("debug_only".to_owned(), serde_json::json!(panel.debug_only));
    component.props.insert(
        "dock_label".to_owned(),
        serde_json::json!(dock_slot_label(&panel.slot_id)),
    );
    component.props.insert(
        "panel_title".to_owned(),
        serde_json::json!(panel.label.as_str()),
    );
    component.props.insert(
        "tooltip".to_owned(),
        serde_json::json!(format!(
            "{} · {}",
            panel.source_gateway, panel.data_contract
        )),
    );
    component
}

fn dock_slot_label(slot: &str) -> &'static str {
    match slot {
        "left.outliner" => "Scene",
        "right.edit_window" => "Inspector",
        "bottom.content_browser" => "Content",
        _ => "Panel",
    }
}

#[allow(clippy::too_many_arguments)]
fn panel<const N: usize>(
    slot_id: &str,
    label: &str,
    surface_id: &str,
    source_gateway: &str,
    data_contract: &str,
    required: bool,
    debug_only: bool,
    tags: [&str; N],
) -> UiScreenPanelDescriptor {
    UiScreenPanelDescriptor {
        slot_id: slot_id.to_owned(),
        label: label.to_owned(),
        surface_id: surface_id.to_owned(),
        source_gateway: source_gateway.to_owned(),
        data_contract: data_contract.to_owned(),
        required,
        debug_only,
        tags: tags.iter().map(|tag| (*tag).to_owned()).collect(),
    }
}

fn screen_metrics(
    descriptor: &UiScreenProfileDescriptor,
    frame_index: u64,
) -> BTreeMap<String, serde_json::Value> {
    BTreeMap::from([
        ("frame_index".to_owned(), serde_json::json!(frame_index)),
        (
            "screen_profile".to_owned(),
            serde_json::json!(descriptor.profile.id()),
        ),
        (
            "layout_id".to_owned(),
            serde_json::json!(descriptor.layout_id.as_str()),
        ),
        (
            "viewport_surface_id".to_owned(),
            serde_json::json!(descriptor.viewport_surface_id.as_str()),
        ),
        (
            "input_focus_policy".to_owned(),
            serde_json::json!(format!("{:?}", descriptor.input_focus_policy)),
        ),
        ("gateway".to_owned(), serde_json::json!("engine.ui")),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn editor_screen_declares_required_editor_panels() {
        let descriptor = editor_screen_descriptor();
        let slots = descriptor
            .panels
            .iter()
            .map(|p| p.slot_id.as_str())
            .collect::<Vec<_>>();
        assert!(slots.contains(&"center.viewport"));
        assert!(slots.contains(&"left.outliner"));
        assert!(slots.contains(&"right.edit_window"));
        assert!(slots.contains(&"bottom.content_browser"));
        assert!(slots.contains(&"bottom.gateway_diagnostics"));
        let content_browser = descriptor
            .panels
            .iter()
            .find(|p| p.slot_id == "bottom.content_browser")
            .unwrap();
        assert_eq!(content_browser.surface_id, "ui.assets.catalog");
        assert_eq!(content_browser.source_gateway, "engine.assets");
        assert_ne!(content_browser.source_gateway, content_browser.surface_id);
        assert!(content_browser
            .tags
            .iter()
            .any(|tag| tag == "ui-composition"));
        assert_eq!(descriptor.profile, UiScreenProfile::Editor);
        assert_eq!(
            descriptor.input_focus_policy,
            UiScreenInputFocusPolicy::EditorShell
        );
    }

    #[test]
    fn game_screen_has_no_editor_panels_by_default() {
        let descriptor = game_screen_descriptor(None);
        assert_eq!(descriptor.profile, UiScreenProfile::Game);
        assert_eq!(
            descriptor.input_focus_policy,
            UiScreenInputFocusPolicy::GameViewport
        );
        assert!(descriptor.panels.is_empty());
        assert!(GameScreen::default().surface_node(1).is_none());
    }

    #[test]
    fn right_edit_window_contract_uses_selection_context_not_native_entity_id() {
        let descriptor = editor_screen_descriptor();
        let edit_window = descriptor
            .panels
            .iter()
            .find(|p| p.slot_id == "right.edit_window")
            .unwrap();
        assert_eq!(
            edit_window.data_contract,
            "newengine.editor.selection_context.v1"
        );
        assert!(edit_window
            .tags
            .iter()
            .any(|tag| tag == "selection-context"));
        assert!(edit_window.tags.iter().any(|tag| tag == "opaque-handles"));
        assert!(!edit_window.data_contract.contains("EntityId"));
        assert!(!edit_window.data_contract.contains("World"));
    }
}
