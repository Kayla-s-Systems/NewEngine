#![forbid(unsafe_op_in_unsafe_fn)]

use crate::ui::{dock, providers, EditorUiBuild, ViewportMode, WorkspacePreset};

impl EditorUiBuild {
    #[inline]
    pub(crate) fn panel_toggle_value(&self, id: providers::UiPanelToggleId) -> bool {
        match id {
            providers::UiPanelToggleId::Outliner => self.layout.show_outliner,
            providers::UiPanelToggleId::Details => self.layout.show_details,
            providers::UiPanelToggleId::LeftToolbar => self.layout.show_left_toolbar,
            providers::UiPanelToggleId::OutputLog => self.layout.show_bottom_console,
            providers::UiPanelToggleId::ContentDrawer => self.layout.show_bottom_content,
        }
    }

    #[inline]
    pub(crate) fn set_panel_toggle(&mut self, id: providers::UiPanelToggleId, value: bool) {
        match id {
            providers::UiPanelToggleId::Outliner => self.layout.show_outliner = value,
            providers::UiPanelToggleId::Details => self.layout.show_details = value,
            providers::UiPanelToggleId::LeftToolbar => self.layout.show_left_toolbar = value,
            providers::UiPanelToggleId::OutputLog => self.layout.show_bottom_console = value,
            providers::UiPanelToggleId::ContentDrawer => self.layout.show_bottom_content = value,
        }
    }

    #[inline]
    pub(crate) fn toggle_panel(&mut self, id: providers::UiPanelToggleId) {
        let next = !self.panel_toggle_value(id);
        self.set_panel_toggle(id, next);
    }

    #[inline]
    pub(crate) fn apply_workspace_preset(&mut self, preset: WorkspacePreset) {
        self.workspace_preset = preset;
        match preset {
            WorkspacePreset::Minimal => {
                self.layout.show_left_toolbar = false;
                self.layout.show_outliner = false;
                self.layout.show_details = true;
                self.layout.show_bottom_console = false;
                self.layout.show_bottom_content = false;
            }
            WorkspacePreset::Editing => {
                self.layout.show_left_toolbar = false;
                self.layout.show_outliner = true;
                self.layout.show_details = true;
                self.layout.show_bottom_console = false;
                self.layout.show_bottom_content = true;
            }
            WorkspacePreset::Debug => {
                self.layout.show_left_toolbar = false;
                self.layout.show_outliner = true;
                self.layout.show_details = true;
                self.layout.show_bottom_console = true;
                self.layout.show_bottom_content = true;
            }
        }
        self.dock_state = dock::dock_state_for_preset(preset);
    }

    #[inline]
    pub(crate) fn set_viewport_mode(&mut self, mode: ViewportMode) {
        match mode {
            ViewportMode::Collision => {
                self.scene_bridge.cmd_set_collision_wireframe(true);
                if matches!(self.viewport_mode, ViewportMode::Collision) {
                    self.viewport_mode = ViewportMode::Lit;
                }
            }
            other => {
                self.viewport_mode = other;
            }
        }
    }
}
