use super::*;

#[derive(Clone, Copy, Debug)]
pub(super) struct EditorChromeDescriptor {
    pub(super) product_title: &'static str,
    pub(super) menu: &'static [EditorChromeMenuItem],
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

pub(super) const EDITOR_CHROME: EditorChromeDescriptor = EditorChromeDescriptor {
    product_title: "North Star",
    menu: EDITOR_MENUS,
    empty_outliner_title: "No scene snapshot",
    empty_outliner_detail: "Scene / World Outliner waits for engine.scene or engine.world snapshot DTO",
    empty_inspector_title: "No selection",
    empty_inspector_detail: "Right Edit Window follows EditorSelectionContext from viewport, outliner or Content Browser",
    viewport_title: "Viewport",
    viewport_detail_edit: "Preview target | simulation stopped",
    viewport_detail_simulate: "Simulation target | player possession disabled",
    viewport_detail_play: "Play target | viewport owns gameplay input",
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
        let state_id = state_id.trim();
        self.states.iter().find(|state| state.id.trim() == state_id)
    }

    pub(super) fn has_action_transition(&self, state_id: &str, action_id: &str) -> bool {
        let state_id = state_id.trim();
        let action_id = action_id.trim();
        self.transitions.iter().any(|transition| {
            transition.from.trim() == state_id
                && transition.on_action.as_deref().map(str::trim) == Some(action_id)
        })
    }

    pub(super) fn validation_errors(&self) -> Vec<String> {
        let mut errors = Vec::new();
        if !self.enabled {
            errors.push("presentation flow is disabled".to_owned());
        }
        if self.id.trim().is_empty() {
            errors.push("flow id is empty".to_owned());
        }
        if self.initial_state.trim().is_empty() {
            errors.push("initial_state is empty".to_owned());
        }

        let mut state_ids = BTreeSet::new();
        for (index, state) in self.states.iter().enumerate() {
            let id = state.id.trim();
            if id.is_empty() {
                errors.push(format!("states[{index}] has an empty id"));
                continue;
            }
            if state.id != id {
                errors.push(format!(
                    "state id '{}' has leading or trailing whitespace",
                    id
                ));
            }
            if !state_ids.insert(id.to_owned()) {
                errors.push(format!("duplicate state id '{id}'"));
            }
            if state
                .document_ref
                .as_ref()
                .is_some_and(|value| value.trim().is_empty())
            {
                errors.push(format!("state '{id}' has an empty document_ref"));
            }
            if state
                .surface_id
                .as_ref()
                .is_some_and(|value| value.trim().is_empty())
            {
                errors.push(format!("state '{id}' has an empty surface_id"));
            }
            if state.document_ref.is_some() != state.surface_id.is_some() {
                errors.push(format!(
                    "state '{id}' must declare document_ref and surface_id together"
                ));
            }
            if state.input_focus_policy == UiScreenInputFocusPolicy::GameViewport
                && state.blocks_gameplay_input
            {
                errors.push(format!(
                    "state '{id}' uses game_viewport focus while blocking gameplay input"
                ));
            }
        }

        let initial_state = self.initial_state.trim();
        if !initial_state.is_empty() && !state_ids.contains(initial_state) {
            errors.push(format!(
                "initial_state '{initial_state}' does not reference a declared state"
            ));
        }

        let mut action_triggers = BTreeSet::new();
        let mut runtime_ready_sources = BTreeSet::new();
        for (index, transition) in self.transitions.iter().enumerate() {
            let from = transition.from.trim();
            let to = transition.to.trim();
            if from.is_empty() {
                errors.push(format!("transitions[{index}] has an empty from state"));
            } else if !state_ids.contains(from) {
                errors.push(format!(
                    "transition {index} from '{from}' references an unknown state"
                ));
            }
            if to.is_empty() {
                errors.push(format!("transitions[{index}] has an empty to state"));
            } else if !state_ids.contains(to) {
                errors.push(format!(
                    "transition {index} to '{to}' references an unknown state"
                ));
            }

            let action = transition.on_action.as_deref().map(str::trim);
            let has_action = action.is_some_and(|value| !value.is_empty());
            if transition.on_action.is_some() && !has_action {
                errors.push(format!("transitions[{index}] has an empty on_action"));
            }
            if has_action == transition.on_runtime_ready {
                errors.push(format!(
                    "transitions[{index}] must have exactly one trigger: on_action or on_runtime_ready"
                ));
            }
            if has_action && !from.is_empty() {
                let action = action.unwrap_or_default();
                if !action_triggers.insert((from.to_owned(), action.to_owned())) {
                    errors.push(format!(
                        "ambiguous action transition from '{from}' on '{action}'"
                    ));
                }
            }
            if transition.on_runtime_ready
                && !from.is_empty()
                && !runtime_ready_sources.insert(from.to_owned())
            {
                errors.push(format!("ambiguous runtime_ready transition from '{from}'"));
            }
        }

        if errors.is_empty() && !state_ids.is_empty() {
            let mut reachable = BTreeSet::new();
            let mut frontier = vec![initial_state.to_owned()];
            while let Some(current) = frontier.pop() {
                if !reachable.insert(current.clone()) {
                    continue;
                }
                for transition in &self.transitions {
                    if transition.from.trim() == current {
                        frontier.push(transition.to.trim().to_owned());
                    }
                }
            }
            for state_id in state_ids.difference(&reachable) {
                errors.push(format!(
                    "state '{state_id}' is unreachable from initial_state '{initial_state}'"
                ));
            }
        }

        errors
    }

    pub(super) fn is_valid(&self) -> bool {
        self.validation_errors().is_empty()
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub(super) struct ScreenProfileConfig {
    #[serde(default = "default_screen_profile")]
    pub(super) profile: UiScreenProfile,
    pub(super) game_ui_root_surface_id: Option<String>,
    pub(super) game_ui_document_ref: Option<String>,
    pub(super) game_gui: Option<UiGameGuiConfig>,
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
            game_gui: None,
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
    pub(super) mounted_game_gui_layers: BTreeMap<String, String>,
    pub(super) failed_game_gui_layers: BTreeSet<String>,
    pub(super) game_gui_visibility_overrides: BTreeMap<String, bool>,
    /// Last visibility actually sent to engine.ui; suppresses redundant retained-layer invalidation.
    pub(super) game_gui_applied_visibility: BTreeMap<String, bool>,
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
    pub(super) hidden_panels: BTreeSet<String>,
    pub(super) last_runtime_command_frame: u64,
    pub(super) last_dock_click_frame: u64,
    pub(super) last_menu_click_frame: u64,
    pub(super) active_menu_id: Option<String>,
    pub(super) last_toast_surface_version: Option<u32>,
    pub(super) last_toast_surface_extent: [u32; 2],
}
