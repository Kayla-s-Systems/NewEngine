#![forbid(unsafe_op_in_unsafe_fn)]

use core::marker::PhantomData;

use newengine_ecs::{EntityId, World};

/// A command produced by a system and applied after execution.
///
/// Commands are committed in deterministic order (by system `(order, seq)`),
/// enabling safe parallel system execution while keeping gameplay deterministic.
pub trait Command: Send {
    /// Apply this command.
    fn apply(self: Box<Self>, world: &mut World);
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
}
