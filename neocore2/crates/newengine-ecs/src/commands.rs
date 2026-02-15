#![forbid(unsafe_op_in_unsafe_fn)]

use crate::EntityId;

/// High-level editor operations are expressed as commands to enable undo/redo later.
pub enum Command {
    Spawn,
    Despawn { entity: EntityId },
    InsertName { entity: EntityId, name: String },
}

/// Command buffer; undo/redo can be built above.
pub struct Commands {
    queue: Vec<Command>,
}

impl Default for Commands {
    #[inline]
    fn default() -> Self {
        Self { queue: Vec::new() }
    }
}

impl Commands {
    #[inline]
    pub fn push(&mut self, c: Command) {
        self.queue.push(c);
    }

    #[inline]
    pub fn drain(&mut self) -> std::vec::IntoIter<Command> {
        core::mem::take(&mut self.queue).into_iter()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.queue.len()
    }

    #[inline]
    pub fn clear(&mut self) {
        self.queue.clear();
    }
}