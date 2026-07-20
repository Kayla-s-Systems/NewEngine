use super::*;

#[derive(Clone, Copy, Debug)]
pub(super) struct EditorChromeDescriptor {
    pub(super) product_title: &'static str,
    pub(super) menu: &'static [EditorChromeMenuItem],
    pub(super) runtime_actions: &'static [EditorRuntimeActionDescriptor],
    pub(super) empty_outliner_title: &'static str,
    pub(super) empty_outliner_detail: &'static str,
    pub(super) empty_inspector_title: &'static str,
    pub(super) empty_inspector_detail: &'static str,
    pub(super) viewport_title: &'static str,
    pub(super) viewport_detail_edit: &'static str,
    pub(super) viewport_detail_simulate: &'static str,
    pub(super) viewport_detail_play: &'static str,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct EditorChromeMenuItem {
    pub(super) id: &'static str,
    pub(super) label: &'static str,
    pub(super) tooltip: &'static str,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct EditorRuntimeActionDescriptor {
    pub(super) id: &'static str,
    pub(super) label: &'static str,
    pub(super) action_id: &'static str,
    pub(super) hotkey: &'static str,
    pub(super) tooltip: &'static str,
    pub(super) mode: UiEditorRuntimeMode,
}

pub(super) const EDITOR_MENUS: &[EditorChromeMenuItem] = &[
    EditorChromeMenuItem {
        id: "file",
        label: "File",
        tooltip: "Project, scene and source operations",
    },
    EditorChromeMenuItem {
        id: "edit",
        label: "Edit",
        tooltip: "Undo, redo and selection operations",
    },
    EditorChromeMenuItem {
        id: "create",
        label: "Create",
        tooltip: "Create entities, assets and authored data",
    },
    EditorChromeMenuItem {
        id: "scene",
        label: "Scene",
        tooltip: "Scene/world commands and placement tools",
    },
    EditorChromeMenuItem {
        id: "assets",
        label: "Assets",
        tooltip: "Import, reimport and inspect content",
    },
    EditorChromeMenuItem {
        id: "tools",
        label: "Tools",
        tooltip: "Editor tools and diagnostics",
    },
    EditorChromeMenuItem {
        id: "window",
        label: "Window",
        tooltip: "Show, hide and dock editor panels",
    },
    EditorChromeMenuItem {
        id: "help",
        label: "Help",
        tooltip: "Documentation and runtime diagnostics",
    },
];

pub(super) const EDITOR_RUNTIME_ACTIONS: &[EditorRuntimeActionDescriptor] = &[
    EditorRuntimeActionDescriptor {
        id: "edit",
        label: "Stop",
        action_id: "editor.runtime.edit",
        hotkey: "1",
        tooltip: "Stop simulation and keep the viewport as an editor preview",
        mode: UiEditorRuntimeMode::Edit,
    },
    EditorRuntimeActionDescriptor {
        id: "simulate",
        label: "Simulate",
        action_id: "editor.runtime.simulate",
        hotkey: "2",
        tooltip: "Run world simulation without direct player possession",
        mode: UiEditorRuntimeMode::Simulate,
    },
    EditorRuntimeActionDescriptor {
        id: "play",
        label: "Play",
        action_id: "editor.runtime.play",
        hotkey: "3",
        tooltip: "Play in editor through the contained viewport",
        mode: UiEditorRuntimeMode::Play,
    },
];

pub(super) const EDITOR_CHROME: EditorChromeDescriptor = EditorChromeDescriptor {
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
pub(super) struct EditorLayoutMetrics {
    pub(super) screen_w: f32,
    pub(super) screen_h: f32,
    pub(super) menu_h: f32,
    pub(super) toolbar_h: f32,
    pub(super) status_h: f32,
    pub(super) bottom_h: f32,
    pub(super) left_w: f32,
    pub(super) right_w: f32,
    pub(super) gap: f32,
    pub(super) viewport_x: f32,
    pub(super) viewport_y: f32,
    pub(super) viewport_w: f32,
    pub(super) viewport_h: f32,
    pub(super) bottom_y: f32,
    pub(super) left_visible: bool,
    pub(super) right_visible: bool,
    pub(super) bottom_visible: bool,
    pub(super) hovered_dock_slot: Option<&'static str>,
    pub(super) hovered_runtime_mode: Option<UiEditorRuntimeMode>,
    pub(super) hovered_menu_id: Option<&'static str>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub(super) struct ScreenPresentationStateConfig {
    pub(super) id: String,
    pub(super) document_ref: Option<String>,
    pub(super) surface_id: Option<String>,
    #[serde(default = "default_ui_surface_focus_policy")]
    pub(super) input_focus_policy: UiScreenInputFocusPolicy,
    pub(super) blocks_world_bootstrap: bool,
    pub(super) blocks_gameplay_input: bool,
}

impl Default for ScreenPresentationStateConfig {
    fn default() -> Self {
        Self {
            id: String::new(),
            document_ref: None,
            surface_id: None,
            input_focus_policy: default_ui_surface_focus_policy(),
            blocks_world_bootstrap: false,
            blocks_gameplay_input: false,
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub(super) struct ScreenPresentationTransitionConfig {
    pub(super) from: String,
    pub(super) to: String,
    pub(super) on_action: Option<String>,
    pub(super) on_runtime_ready: bool,
    pub(super) reset_runtime_ready: bool,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub(super) struct ScreenPresentationFlowConfig {
    pub(super) enabled: bool,
    pub(super) id: String,
    pub(super) initial_state: String,
    pub(super) states: Vec<ScreenPresentationStateConfig>,
    pub(super) transitions: Vec<ScreenPresentationTransitionConfig>,
}

impl ScreenPresentationFlowConfig {
    pub(super) fn state(&self, state_id: &str) -> Option<&ScreenPresentationStateConfig> {
        self.states.iter().find(|state| state.id == state_id)
    }

    pub(super) fn is_valid(&self) -> bool {
        self.enabled
            && !self.id.trim().is_empty()
            && !self.initial_state.trim().is_empty()
            && self.state(self.initial_state.trim()).is_some()
            && self.states.iter().all(|state| !state.id.trim().is_empty())
            && self.transitions.iter().all(|transition| {
                self.state(transition.from.trim()).is_some()
                    && self.state(transition.to.trim()).is_some()
                    && (transition
                        .on_action
                        .as_deref()
                        .is_some_and(|action| !action.trim().is_empty())
                        || transition.on_runtime_ready)
            })
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub(super) struct ScreenProfileConfig {
    #[serde(default = "default_screen_profile")]
    pub(super) profile: UiScreenProfile,
    pub(super) game_ui_root_surface_id: Option<String>,
    pub(super) game_ui_document_ref: Option<String>,
    pub(super) presentation_flow: Option<ScreenPresentationFlowConfig>,
    #[serde(default = "default_publish_editor_shell")]
    pub(super) publish_editor_shell: bool,
}

pub(super) fn default_screen_profile() -> UiScreenProfile {
    UiScreenProfile::Editor
}

pub(super) fn default_publish_editor_shell() -> bool {
    true
}

pub(super) fn default_ui_surface_focus_policy() -> UiScreenInputFocusPolicy {
    UiScreenInputFocusPolicy::UiSurface
}

impl Default for ScreenProfileConfig {
    fn default() -> Self {
        Self {
            // The North Star desktop host is an editor-first runtime. Game presentation
            // remains available through explicit profile config, but the default boot
            // surface must expose the editor shell, content browser and right edit window.
            profile: UiScreenProfile::Editor,
            game_ui_root_surface_id: None,
            game_ui_document_ref: None,
            presentation_flow: None,
            publish_editor_shell: true,
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct ScreenProfileRuntimeState {
    pub(super) config: ScreenProfileConfig,
    pub(super) descriptor: UiScreenProfileDescriptor,
    pub(super) last_published_profile: Option<UiScreenProfile>,
    pub(super) published_surfaces: BTreeSet<String>,
    pub(super) mounted_game_ui_document_ref: Option<String>,
    pub(super) failed_game_ui_document_ref: Option<String>,
    pub(super) presentation_state_id: Option<String>,
    pub(super) last_published_presentation_state_id: Option<String>,
    pub(super) presentation_runtime_ready: bool,
    pub(super) mounted_presentation_documents: BTreeMap<String, String>,
    pub(super) failed_presentation_documents: BTreeSet<String>,
    pub(super) last_presentation_action_frame: u64,
    pub(super) pending_presentation_action_id: Option<String>,
    pub(super) pending_presentation_action_frame: u64,
    pub(super) last_right_edit_selection_key: String,
    pub(super) cached_right_edit_document: Option<AssetDocument>,
    pub(super) cached_right_edit_error: Option<String>,
    pub(super) editor_runtime_mode: UiEditorRuntimeMode,
    pub(super) hidden_panels: BTreeSet<String>,
    pub(super) last_runtime_button_pointer_frame: u64,
    pub(super) last_dock_click_frame: u64,
    pub(super) last_menu_click_frame: u64,
    pub(super) active_menu_id: Option<String>,
}
