#![forbid(unsafe_op_in_unsafe_fn)]

use core::marker::PhantomData;

#[cfg(debug_assertions)]
use core::any::TypeId;

use newengine_ecs::{EntityId, World};

/// Debug classification for command validation.
#[cfg(debug_assertions)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommandTag {
    Insert {
        type_id: TypeId,
        type_name: &'static str,
    },
    Remove {
        type_id: TypeId,
        type_name: &'static str,
    },
    /// A sanctioned transform write path (via intent/patch command).
    TransformWrite,
    /// Appends a controller-produced intent batch into the world queue.
    IntentQueueAppend,
    /// Clears the world intent queue after the dedicated apply stage has consumed it.
    IntentQueueClear,
    Other(&'static str),
}

/// A command produced by a system and applied after execution.
///
/// Commands are committed in deterministic order (by system `(order, seq)`).
/// If this stage becomes parallel again, it must run through `engine.threading` so
/// each batch remains visible and controllable.
pub trait Command: Send {
    /// Apply this command.
    fn apply(self: Box<Self>, world: &mut World);

    /// Debug tag used for stage validation.
    #[cfg(debug_assertions)]
    #[inline]
    fn tag(&self) -> CommandTag {
        CommandTag::Other(core::any::type_name::<Self>())
    }
}

/// Per-system command buffer.
#[derive(Default)]
pub struct CommandBuffer {
    cmds: Vec<Box<dyn Command>>,
}

impl CommandBuffer {
    #[inline]
    pub fn new() -> Self {
        Self { cmds: Vec::new() }
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.cmds.is_empty()
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.cmds.len()
    }

    #[inline]
    pub fn push(&mut self, cmd: Box<dyn Command>) {
        self.cmds.push(cmd);
    }

    #[inline]
    pub fn extend(&mut self, other: CommandBuffer) {
        self.cmds.extend(other.cmds);
    }

    #[inline]
    pub fn apply_all(self, world: &mut World) {
        for cmd in self.cmds {
            cmd.apply(world);
        }
    }

    #[cfg(debug_assertions)]
    #[inline]
    pub(crate) fn iter(&self) -> impl Iterator<Item = &dyn Command> {
        self.cmds.iter().map(|c| &**c)
    }

    #[inline]
    pub fn insert<T>(&mut self, entity: EntityId, value: T)
    where
        T: Send + Sync + 'static,
    {
        self.push(Box::new(InsertCmd { entity, value }));
    }

    #[inline]
    pub fn remove<T>(&mut self, entity: EntityId)
    where
        T: Send + Sync + 'static,
    {
        self.push(Box::new(RemoveCmd::<T>::new(entity)));
    }
}

struct InsertCmd<T>
where
    T: Send + Sync + 'static,
{
    entity: EntityId,
    value: T,
}

impl<T> Command for InsertCmd<T>
where
    T: Send + Sync + 'static,
{
    #[inline]
    fn apply(self: Box<Self>, world: &mut World) {
        let _ = world.insert(self.entity, self.value);
    }

    #[cfg(debug_assertions)]
    #[inline]
    fn tag(&self) -> CommandTag {
        CommandTag::Insert {
            type_id: TypeId::of::<T>(),
            type_name: core::any::type_name::<T>(),
        }
    }
}

struct RemoveCmd<T>
where
    T: Send + Sync + 'static,
{
    entity: EntityId,
    _marker: PhantomData<fn() -> T>,
}

impl<T> RemoveCmd<T>
where
    T: Send + Sync + 'static,
{
    #[inline]
    pub fn new(entity: EntityId) -> Self {
        Self {
            entity,
            _marker: PhantomData,
        }
    }
}

impl<T> Command for RemoveCmd<T>
where
    T: Send + Sync + 'static,
{
    #[inline]
    fn apply(self: Box<Self>, world: &mut World) {
        let _ = world.remove::<T>(self.entity);
    }

    #[cfg(debug_assertions)]
    #[inline]
    fn tag(&self) -> CommandTag {
        CommandTag::Remove {
            type_id: TypeId::of::<T>(),
            type_name: core::any::type_name::<T>(),
        }
    }
}
