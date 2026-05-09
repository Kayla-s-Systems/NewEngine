#![forbid(unsafe_op_in_unsafe_fn)]

use crate::ui::{dock, EditorUiBuild, ViewportMode, WorkspacePreset};

impl EditorUiBuild {
    #[inline]
    pub(crate) fn apply_workspace_preset(&mut self, preset: WorkspacePreset) {
        self.workspace_preset = preset;
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
