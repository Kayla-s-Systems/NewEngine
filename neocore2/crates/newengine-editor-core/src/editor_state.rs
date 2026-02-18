#![forbid(unsafe_op_in_unsafe_fn)]

use crate::{CommandStack, SelectionModel};

/// High-level gizmo mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GizmoMode {
    Translate,
    Rotate,
    Scale,
}

/// High-level editor tool.
///
/// Tools may share the same gizmo mode (e.g. a custom placement tool can use Translate gizmo).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ToolId {
    Select,
    Translate,
    Rotate,
    Scale,
}

/// Editor-wide state container.
///
/// This is the single source of truth for selection, tool modes and undo/redo.
#[derive(Debug, Clone)]
pub struct EditorState {
    pub selection: SelectionModel,
    pub commands: CommandStack,
    pub gizmo_mode: GizmoMode,
    pub active_tool: ToolId,
}

impl EditorState {
    #[inline]
    pub fn new() -> Self {
        Self {
            selection: SelectionModel::default(),
            commands: CommandStack::default(),
            gizmo_mode: GizmoMode::Translate,
            active_tool: ToolId::Select,
        }
    }
}

impl Default for EditorState {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}
