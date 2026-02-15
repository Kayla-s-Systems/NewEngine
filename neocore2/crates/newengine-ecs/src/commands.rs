#![forbid(unsafe_op_in_unsafe_fn)]

use crate::{EntityId, World};

/// Placeholder for an entity that will be spawned when the command buffer is applied.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct EntityToken(u32);

impl EntityToken {
    #[inline]
    pub fn index(self) -> u32 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum EntityTarget {
    Existing(EntityId),
    Token(EntityToken),
}

/// Deterministic ECS command buffer.
///
/// Principles:
/// - commands are recorded in-order
/// - applying is a strict two-phase commit:
///   1) spawn all entities to resolve `EntityToken`
///   2) apply remaining operations in original order
///
/// This keeps mutation explicit and prevents borrow hazards inside systems.
pub struct Commands {
    next_token: u32,
    ops: Vec<Op>,
}

enum Op {
    Spawn { token: EntityToken },
    Despawn { target: EntityTarget },
    Insert {
        target: EntityTarget,
        f: Box<dyn FnOnce(&mut World, EntityId) + Send + 'static>,
    },
    Remove {
        target: EntityTarget,
        f: Box<dyn FnOnce(&mut World, EntityId) + Send + 'static>,
    },
}

impl Default for Commands {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl Commands {
    #[inline]
    pub fn new() -> Self {
        Self {
            next_token: 1,
            ops: Vec::new(),
        }
    }

    /// Records a spawn and returns a placeholder token.
    #[inline]
    pub fn spawn(&mut self) -> EntityToken {
        let t = EntityToken(self.next_token);
        self.next_token = self.next_token.wrapping_add(1).max(1);
        self.ops.push(Op::Spawn { token: t });
        t
    }

    #[inline]
    pub fn despawn(&mut self, entity: EntityId) {
        self.ops.push(Op::Despawn {
            target: EntityTarget::Existing(entity),
        });
    }

    #[inline]
    pub fn despawn_token(&mut self, token: EntityToken) {
        self.ops.push(Op::Despawn {
            target: EntityTarget::Token(token),
        });
    }

    /// Inserts/replaces a component on an existing entity.
    #[inline]
    pub fn insert<T>(&mut self, entity: EntityId, component: T)
    where
        T: crate::Component,
    {
        self.ops.push(Op::Insert {
            target: EntityTarget::Existing(entity),
            f: Box::new(move |w, id| {
                let _ = w.insert(id, component);
            }),
        });
    }

    /// Inserts/replaces a component on a token entity.
    #[inline]
    pub fn insert_token<T>(&mut self, token: EntityToken, component: T)
    where
        T: crate::Component,
    {
        self.ops.push(Op::Insert {
            target: EntityTarget::Token(token),
            f: Box::new(move |w, id| {
                let _ = w.insert(id, component);
            }),
        });
    }

    /// Removes a component from an existing entity.
    #[inline]
    pub fn remove<T>(&mut self, entity: EntityId)
    where
        T: crate::Component,
    {
        self.ops.push(Op::Remove {
            target: EntityTarget::Existing(entity),
            f: Box::new(move |w, id| {
                let _ = w.remove::<T>(id);
            }),
        });
    }

    /// Removes a component from a token entity.
    #[inline]
    pub fn remove_token<T>(&mut self, token: EntityToken)
    where
        T: crate::Component,
    {
        self.ops.push(Op::Remove {
            target: EntityTarget::Token(token),
            f: Box::new(move |w, id| {
                let _ = w.remove::<T>(id);
            }),
        });
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.ops.is_empty()
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.ops.len()
    }

    #[inline]
    pub fn clear(&mut self) {
        self.ops.clear();
    }

    /// Applies all recorded commands to the world.
    ///
    /// Returns a dense mapping of `EntityToken.index()` -> `EntityId`.
    /// The mapping is valid only for tokens created by this `Commands` instance.
    pub fn apply(&mut self, world: &mut World) -> Vec<(EntityToken, EntityId)> {
        if self.ops.is_empty() {
            return Vec::new();
        }

        // Phase 1: resolve spawns.
        let mut spawned: Vec<(EntityToken, EntityId)> = Vec::new();
        for op in self.ops.iter() {
            if let Op::Spawn { token } = *op {
                let id = world.spawn();
                spawned.push((token, id));
            }
        }

        // Helper for token resolution (linear scan is fine for editor/early runtime;
        // can be replaced by a small hashmap if needed later).
        let mut resolve = |t: EntityToken| -> Option<EntityId> {
            spawned
                .iter()
                .find(|(tok, _)| *tok == t)
                .map(|(_, id)| *id)
        };

        // Phase 2: apply in original order.
        let ops = core::mem::take(&mut self.ops);
        for op in ops {
            match op {
                Op::Spawn { .. } => {}
                Op::Despawn { target } => {
                    let id = match target {
                        EntityTarget::Existing(e) => Some(e),
                        EntityTarget::Token(t) => resolve(t),
                    };
                    if let Some(id) = id {
                        let _ = world.despawn(id);
                    }
                }
                Op::Insert { target, f } => {
                    let id = match target {
                        EntityTarget::Existing(e) => Some(e),
                        EntityTarget::Token(t) => resolve(t),
                    };
                    if let Some(id) = id {
                        f(world, id);
                    }
                }
                Op::Remove { target, f } => {
                    let id = match target {
                        EntityTarget::Existing(e) => Some(e),
                        EntityTarget::Token(t) => resolve(t),
                    };
                    if let Some(id) = id {
                        f(world, id);
                    }
                }
            }
        }

        spawned
    }
}
