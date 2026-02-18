#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_ecs::EntityId;
use newengine_math::Vec3;

/// Snapshot of an entity local transform in editor-friendly representation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TransformSnapshot {
    pub position: Vec3,
    pub rotation_ypr: (f32, f32, f32),
    pub scale: Vec3,
}

impl TransformSnapshot {
    #[inline]
    pub fn new(position: Vec3, rotation_ypr: (f32, f32, f32), scale: Vec3) -> Self {
        Self {
            position,
            rotation_ypr,
            scale,
        }
    }
}

/// Editor command that supports deterministic undo/redo.
#[derive(Debug, Clone, PartialEq)]
pub enum EditorCommand {
    SetTransform {
        entity: EntityId,
        before: TransformSnapshot,
        after: TransformSnapshot,
    },
}

impl EditorCommand {
    #[inline]
    pub fn entity(&self) -> EntityId {
        match *self {
            EditorCommand::SetTransform { entity, .. } => entity,
        }
    }
}

/// Simple deterministic undo/redo stack.
#[derive(Debug, Default, Clone)]
pub struct CommandStack {
    undo: Vec<EditorCommand>,
    redo: Vec<EditorCommand>,
}

impl CommandStack {
    #[inline]
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    #[inline]
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    #[inline]
    pub fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
    }

    #[inline]
    pub fn push(&mut self, cmd: EditorCommand) {
        self.undo.push(cmd);
        self.redo.clear();
    }

    #[inline]
    pub fn pop_undo(&mut self) -> Option<EditorCommand> {
        let cmd = self.undo.pop()?;
        self.redo.push(cmd.clone());
        Some(cmd)
    }

    #[inline]
    pub fn pop_redo(&mut self) -> Option<EditorCommand> {
        let cmd = self.redo.pop()?;
        self.undo.push(cmd.clone());
        Some(cmd)
    }

    #[inline]
    pub fn len_undo(&self) -> usize {
        self.undo.len()
    }

    #[inline]
    pub fn len_redo(&self) -> usize {
        self.redo.len()
    }
}
