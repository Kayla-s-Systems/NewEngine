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

pub struct Storage<T: Component> {
    pub(crate) map: SecondaryMap<EntityId, T>,
}

impl<T: Component> Storage<T> {
    #[inline]
    pub fn new() -> Self {
        Self {
            map: SecondaryMap::new(),
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
        self.map.remove(id);
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