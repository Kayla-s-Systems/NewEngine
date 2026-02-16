#![forbid(unsafe_op_in_unsafe_fn)]

use crate::{EntityId, World};

/// Opaque handle for an entity that will be spawned when the command buffer is applied.
///
/// # Determinism
/// The token is deterministic within the lifetime of a single [`Commands`] buffer.
/// It is **not** a persistent identifier.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct EntityToken(u32);

impl EntityToken {
    /// Stable monotonic index (starting from 1) inside a single command buffer.
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
/// ## Principles
/// - Commands are recorded in-order.
/// - Applying is a strict two-phase commit:
///   1) spawn all entities to resolve [`EntityToken`]
///   2) apply remaining operations in the original order
///
/// This keeps mutation explicit and prevents borrow hazards inside systems.
///
/// ## Important semantics
/// - Tokens are only valid for the *current* contents of the buffer.
/// - If you record a token but never spawn it (i.e. no `spawn()` call producing it),
///   it will never resolve during `apply()`.
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

    /// Records a spawn and returns an opaque token.
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
        // Tokens are only meaningful while ops are alive.
        // Resetting keeps memory usage predictable across frames.
        self.next_token = 1;
    }

    /// Applies all recorded commands to the world.
    ///
    /// Returns a monotonic list of resolved `(EntityToken, EntityId)` pairs for all tokens
    /// created via [`Commands::spawn`] in this buffer.
    pub fn apply(&mut self, world: &mut World) -> Vec<(EntityToken, EntityId)> {
        if self.ops.is_empty() {
            return Vec::new();
        }

        // Phase 1: resolve spawns.
        //
        // Tokens are monotonic indices starting at 1, so we can resolve in O(1)
        // via a vector where index = token.index() - 1.
        //
        // IMPORTANT: tokens can be sparse (e.g. if ops are manipulated externally in the future
        // or if we ever add conditional recording). We therefore store `Option<EntityId>` to
        // avoid fabricating mappings for tokens that never spawned.
        let mut token_to_entity: Vec<Option<EntityId>> = Vec::new();
        token_to_entity.reserve(self.next_token.saturating_sub(1) as usize);

        for op in self.ops.iter() {
            if let Op::Spawn { token } = *op {
                let id = world.spawn();
                let idx = (token.index().saturating_sub(1)) as usize;
                if token_to_entity.len() <= idx {
                    token_to_entity.resize(idx + 1, None);
                }
                token_to_entity[idx] = Some(id);
            }
        }

        #[inline]
        fn resolve(t: EntityToken, map: &[Option<EntityId>]) -> Option<EntityId> {
            let idx = (t.index().saturating_sub(1)) as usize;
            map.get(idx).and_then(|v| *v)
        }

        // Phase 2: apply in original order.
        let ops = core::mem::take(&mut self.ops);
        // The command buffer is now empty; reset token counter to avoid unbounded growth.
        self.next_token = 1;

        for op in ops {
            match op {
                Op::Spawn { .. } => {}
                Op::Despawn { target } => {
                    let id = match target {
                        EntityTarget::Existing(e) => Some(e),
                        EntityTarget::Token(t) => resolve(t, &token_to_entity),
                    };
                    if let Some(id) = id {
                        let _ = world.despawn(id);
                    }
                }
                Op::Insert { target, f } => {
                    let id = match target {
                        EntityTarget::Existing(e) => Some(e),
                        EntityTarget::Token(t) => resolve(t, &token_to_entity),
                    };
                    if let Some(id) = id {
                        f(world, id);
                    }
                }
                Op::Remove { target, f } => {
                    let id = match target {
                        EntityTarget::Existing(e) => Some(e),
                        EntityTarget::Token(t) => resolve(t, &token_to_entity),
                    };
                    if let Some(id) = id {
                        f(world, id);
                    }
                }
            }
        }

        // Public return: resolved spawn tokens only, in token order.
        let mut out: Vec<(EntityToken, EntityId)> = Vec::new();
        out.reserve(token_to_entity.len());
        for (i, id) in token_to_entity.into_iter().enumerate() {
            if let Some(id) = id {
                out.push((EntityToken((i as u32) + 1), id));
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn apply_returns_only_spawned_tokens_and_keeps_order() {
        let mut w = World::new();
        let mut cmd = Commands::new();

        let t1 = cmd.spawn();
        let t2 = cmd.spawn();
        // Apply.
        let map = cmd.apply(&mut w);
        assert_eq!(map.len(), 2);
        assert_eq!(map[0].0, t1);
        assert_eq!(map[1].0, t2);
        assert!(w.exists(map[0].1));
        assert!(w.exists(map[1].1));
    }

    #[test]
    fn token_resolution_is_not_fabricated_for_missing_indices() {
        let mut w = World::new();
        let mut cmd = Commands::new();

        let t1 = cmd.spawn();
        let t2 = cmd.spawn();

        // Clear ops without resetting next_token would be a future hazard; our API resets it.
        // Simulate a sparse token vector by recording extra tokens and removing ops.
        // We can't legally create a gap via public API today, but this test protects
        // against regressions if the recording model changes.
        cmd.ops.pop(); // remove spawn t2

        let map = cmd.apply(&mut w);
        assert_eq!(map.len(), 1);
        assert_eq!(map[0].0, t1);

        // t2 must not resolve to anything.
        let mut cmd2 = Commands::new();
        cmd2.despawn_token(t2);
        let _ = cmd2.apply(&mut w);
        // no panic is the key property here
    }
}
