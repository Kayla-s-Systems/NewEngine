#[derive(Clone, Copy, Debug)]
struct EditorChromeDescriptor {
    product_title: &'static str,
    menu: &'static [EditorChromeMenuItem],
    runtime_actions: &'static [EditorRuntimeActionDescriptor],
    empty_outliner_title: &'static str,
    empty_outliner_detail: &'static str,
    empty_inspector_title: &'static str,
    empty_inspector_detail: &'static str,
    viewport_title: &'static str,
    viewport_detail_edit: &'static str,
    viewport_detail_simulate: &'static str,
    viewport_detail_play: &'static str,
}

#[derive(Clone, Copy, Debug)]
struct EditorChromeMenuItem {
    id: &'static str,
    label: &'static str,
    tooltip: &'static str,
}

#[derive(Clone, Copy, Debug)]
struct EditorRuntimeActionDescriptor {
    id: &'static str,
    label: &'static str,
    action_id: &'static str,
    hotkey: &'static str,
    tooltip: &'static str,
    mode: UiEditorRuntimeMode,
}

const EDITOR_MENUS: &[EditorChromeMenuItem] = &[
    EditorChromeMenuItem { id: "file", label: "File", tooltip: "Project, scene and source operations" },
    EditorChromeMenuItem { id: "edit", label: "Edit", tooltip: "Undo, redo and selection operations" },
    EditorChromeMenuItem { id: "create", label: "Create", tooltip: "Create entities, assets and authored data" },
    EditorChromeMenuItem { id: "scene", label: "Scene", tooltip: "Scene/world commands and placement tools" },
    EditorChromeMenuItem { id: "assets", label: "Assets", tooltip: "Import, reimport and inspect content" },
    EditorChromeMenuItem { id: "tools", label: "Tools", tooltip: "Editor tools and diagnostics" },
    EditorChromeMenuItem { id: "window", label: "Window", tooltip: "Show, hide and dock editor panels" },
    EditorChromeMenuItem { id: "help", label: "Help", tooltip: "Documentation and runtime diagnostics" },
];

const EDITOR_RUNTIME_ACTIONS: &[EditorRuntimeActionDescriptor] = &[
    EditorRuntimeActionDescriptor { id: "edit", label: "Stop", action_id: "editor.runtime.edit", hotkey: "1", tooltip: "Stop simulation and keep the viewport as an editor preview", mode: UiEditorRuntimeMode::Edit },
    EditorRuntimeActionDescriptor { id: "simulate", label: "Simulate", action_id: "editor.runtime.simulate", hotkey: "2", tooltip: "Run world simulation without direct player possession", mode: UiEditorRuntimeMode::Simulate },
    EditorRuntimeActionDescriptor { id: "play", label: "Play", action_id: "editor.runtime.play", hotkey: "3", tooltip: "Play in editor through the contained viewport", mode: UiEditorRuntimeMode::Play },
];

const EDITOR_CHROME: EditorChromeDescriptor = EditorChromeDescriptor {
    product_title: "North Star",
    menu: EDITOR_MENUS,
    runtime_actions: EDITOR_RUNTIME_ACTIONS,
    empty_outliner_title: "No scene snapshot",
    empty_outliner_detail: "Scene / World Outliner waits for engine.scene or engine.world snapshot DTO",
    empty_inspector_title: "No selection",
    empty_inspector_detail: "Right Edit Window follows EditorSelectionContext from viewport, outliner or Content Browser",
    viewport_title: "Viewport",
    viewport_detail_edit: "Preview target · simulation stopped",
    viewport_detail_simulate: "Simulation target · player possession disabled",
    viewport_detail_play: "Play target · viewport owns gameplay input",
};


#[derive(Clone, Copy, Debug)]
struct EditorLayoutMetrics {
    screen_w: f32,
    screen_h: f32,
    menu_h: f32,
    toolbar_h: f32,
    status_h: f32,
    bottom_h: f32,
    left_w: f32,
    right_w: f32,
    gap: f32,
    viewport_x: f32,
    viewport_y: f32,
    viewport_w: f32,
    viewport_h: f32,
    bottom_y: f32,
    left_visible: bool,
    right_visible: bool,
    bottom_visible: bool,
    hovered_dock_slot: Option<&'static str>,
    hovered_runtime_mode: Option<UiEditorRuntimeMode>,
    hovered_menu_id: Option<&'static str>,
}


#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
struct ScreenProfileConfig {
    #[serde(default = "default_screen_profile")]
    profile: UiScreenProfile,
    game_ui_root_surface_id: Option<String>,
    #[serde(default = "default_publish_editor_shell")]
    publish_editor_shell: bool,
}

fn default_screen_profile() -> UiScreenProfile {
    UiScreenProfile::Editor
}

fn default_publish_editor_shell() -> bool {
    true
}

impl Default for ScreenProfileConfig {
    fn default() -> Self {
        Self {
            // The North Star desktop host is an editor-first runtime. Game presentation
            // remains available through explicit profile config, but the default boot
            // surface must expose the editor shell, content browser and right edit window.
            profile: UiScreenProfile::Editor,
            game_ui_root_surface_id: None,
            publish_editor_shell: true,
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct ScreenProfileRuntimeState {
    config: ScreenProfileConfig,
    descriptor: UiScreenProfileDescriptor,
    last_published_profile: Option<UiScreenProfile>,
    published_surfaces: BTreeSet<String>,
    last_right_edit_selection_key: String,
    cached_right_edit_document: Option<AssetDocument>,
    cached_right_edit_error: Option<String>,
    editor_runtime_mode: UiEditorRuntimeMode,
    hidden_panels: BTreeSet<String>,
    last_runtime_button_pointer_frame: u64,
    last_dock_click_frame: u64,
    last_menu_click_frame: u64,
    active_menu_id: Option<String>,
}

