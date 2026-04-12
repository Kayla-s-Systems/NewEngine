#![forbid(unsafe_op_in_unsafe_fn)]

use crate::ui::{dock, EditorUiBuild};

impl EditorUiBuild {
    #[inline]
    pub(crate) fn ensure_dock_tab_open(&mut self, tab: dock::EditorDockTab) {
        if self.dock_state.find_tab(&tab).is_none() {
            self.dock_state.main_surface_mut().push_to_focused_leaf(tab);
        }
    }

    #[inline]
    pub(crate) fn reset_dock_layout(&mut self) {
        self.dock_state = dock::dock_state_for_preset(self.workspace_preset);
    }

    #[inline]
    pub(crate) fn save_dock_layout_snapshot(&mut self) {
        self.saved_dock_layout = Some(self.dock_state.clone());
    }

    #[inline]
    pub(crate) fn restore_dock_layout_snapshot(&mut self) {
        if let Some(saved) = self.saved_dock_layout.clone() {
            self.dock_state = saved;
        }
    }
}
