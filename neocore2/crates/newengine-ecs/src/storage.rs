#![forbid(unsafe_op_in_unsafe_fn)]

use core::any::{Any, TypeId};

use slotmap::SecondaryMap;

use crate::{Component, EntityId};

/// Type-erased component storage stored inside `World`.
///
/// `Send + Sync` are required to make `World` thread-safe.
pub trait ErasedStorage: Any + Send + Sync {
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;

    fn component_type_id(&self) -> TypeId;

    fn remove_entity(&mut self, id: EntityId);
    fn has(&self, id: EntityId) -> bool;

    fn len(&self) -> usize;
}

/// Per-component storage with conservative change tracking.
///
/// Tracking rules:
/// - `insert` marks `added` (if new) and `changed`.
/// - `get_mut` conservatively marks `changed`.
/// - `remove` does not emit events by itself (use `Events<T>` or a higher-level log).
pub struct Storage<T: Component> {
    pub(crate) map: SecondaryMap<EntityId, T>,
    pub(crate) added_tick: SecondaryMap<EntityId, u64>,
    pub(crate) changed_tick: SecondaryMap<EntityId, u64>,
}

impl<T: Component> Storage<T> {
    #[inline]
    pub fn new() -> Self {
        Self {
            map: SecondaryMap::new(),
            added_tick: SecondaryMap::new(),
            changed_tick: SecondaryMap::new(),
        }
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.map.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    #[inline]
    pub fn added_tick(&self, id: EntityId) -> Option<u64> {
        self.added_tick.get(id).copied()
    }

    #[inline]
    pub fn changed_tick(&self, id: EntityId) -> Option<u64> {
        self.changed_tick.get(id).copied()
    }

    #[inline]
    pub fn mark_changed(&mut self, id: EntityId, tick: u64) {
        if self.map.contains_key(id) {
            self.changed_tick.insert(id, tick);
        }
    }

    #[inline]
    pub fn mark_added(&mut self, id: EntityId, tick: u64) {
        if self.map.contains_key(id) {
            self.added_tick.insert(id, tick);
            self.changed_tick.insert(id, tick);
        }
    }

    #[inline]
    pub fn remove_all_traces(&mut self, id: EntityId) {
        let _ = self.map.remove(id);
        let _ = self.added_tick.remove(id);
        let _ = self.changed_tick.remove(id);
    }
}

impl<T: Component> Default for Storage<T> {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Component> ErasedStorage for Storage<T> {
    #[inline]
    fn as_any(&self) -> &dyn Any {
        self
    }

    #[inline]
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    #[inline]
    fn component_type_id(&self) -> TypeId {
        TypeId::of::<T>()
    }

    #[inline]
    fn remove_entity(&mut self, id: EntityId) {
        self.remove_all_traces(id);
    }

    #[inline]
    fn has(&self, id: EntityId) -> bool {
        self.map.contains_key(id)
    }

    #[inline]
    fn len(&self) -> usize {
        self.map.len()
    }
}
