#![forbid(unsafe_op_in_unsafe_fn)]

use std::collections::VecDeque;

use newengine_editor_core::ToolId;

use crate::gameplay::EditorPlayMode;

use super::super::schema::AssetSpawnContract;
use super::super::{providers, schema, ViewportMode, WorkspacePreset};

#[derive(Debug, Clone)]
pub(crate) enum TypedEditorCommand {
    UiAction(providers::UiAction),
    ContextAction(schema::ContextActionId),
    SpawnAsset { contract: AssetSpawnContract, source: &'static str },
    SetTool(ToolId),
    SetPlayMode(EditorPlayMode),
    SetWorkspacePreset(WorkspacePreset),
    SetViewportMode(ViewportMode),
    PublishFrameSelection,
    PublishFrameAll,
    ToggleCollisionOverlay,
}

#[derive(Debug, Default)]
pub(crate) struct EditorCommandBus {
    queue: VecDeque<TypedEditorCommand>,
}

impl EditorCommandBus {
    #[inline]
    pub(crate) fn push(&mut self, cmd: TypedEditorCommand) {
        self.queue.push_back(cmd);
    }

    #[inline]
    pub(crate) fn pop(&mut self) -> Option<TypedEditorCommand> {
        self.queue.pop_front()
    }

    #[inline]
    pub(crate) fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    #[inline]
    pub(crate) fn len(&self) -> usize {
        self.queue.len()
    }
}
