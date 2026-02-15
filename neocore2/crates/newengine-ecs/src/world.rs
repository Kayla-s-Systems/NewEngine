#![forbid(unsafe_op_in_unsafe_fn)]

use core::any::{Any, TypeId};

use hashbrown::HashMap;
use slotmap::SlotMap;

use crate::{
    query::{Query, Query2, Query2A, Query2B, QueryMut}, storage::{ErasedStorage, Storage},
    Component,
    EntityId,
};

/// A small, deterministic ECS world.
///
/// Design goals:
/// - deterministic entity identity via generational keys
/// - type-safe component storage
/// - iteration without hidden allocations (iterators are thin wrappers)
/// - thread-safe storages/resources (Send + Sync), so scene bridges can safely share it
pub struct World {
    entities: SlotMap<EntityId, ()>,
    storages: HashMap<TypeId, Box<dyn ErasedStorage>>,
    resources: HashMap<TypeId, Box<dyn Any + Send + Sync>>,
}

impl Default for World {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl World {
    #[inline]
    pub fn new() -> Self {
        Self {
            entities: SlotMap::with_key(),
            storages: HashMap::new(),
            resources: HashMap::new(),
        }
    }

    #[inline]
    pub fn spawn(&mut self) -> EntityId {
        self.entities.insert(())
    }

    #[inline]
    pub fn exists(&self, id: EntityId) -> bool {
        self.entities.contains_key(id)
    }

    #[inline]
    pub fn entity_count(&self) -> usize {
        self.entities.len()
    }

    /// Despawns an entity and removes all its components.
    #[inline]
    pub fn despawn(&mut self, id: EntityId) -> bool {
        if self.entities.remove(id).is_none() {
            return false;
        }
        for s in self.storages.values_mut() {
            s.remove_entity(id);
        }
        true
    }

    #[inline]
    pub fn iter_entities(&self) -> impl Iterator<Item=EntityId> + '_ {
        self.entities.keys()
    }

    // -----------------------------
    // Resources (singletons)
    // -----------------------------

    /// Inserts (or replaces) a resource.
    #[inline]
    pub fn insert_resource<T: 'static + Send + Sync>(&mut self, r: T) {
        self.resources
            .insert(TypeId::of::<T>(), Box::new(r));
    }

    /// Returns an immutable resource reference.
    #[inline]
    pub fn resource<T: 'static + Send + Sync>(&self) -> Option<&T> {
        self.resources
            .get(&TypeId::of::<T>())
            .and_then(|b| b.downcast_ref::<T>())
    }

    /// Returns a mutable resource reference.
    #[inline]
    pub fn resource_mut<T: 'static + Send + Sync>(&mut self) -> Option<&mut T> {
        self.resources
            .get_mut(&TypeId::of::<T>())
            .and_then(|b| b.downcast_mut::<T>())
    }

    /// Removes a resource.
    #[inline]
    pub fn remove_resource<T: 'static + Send + Sync>(&mut self) -> Option<T> {
        self.resources
            .remove(&TypeId::of::<T>())
            .and_then(|b| b.downcast::<T>().ok())
            .map(|b| *b)
    }

    // -----------------------------
    // Component storages
    // -----------------------------

    /// Ensures storage for component `T` exists and returns mutable access to it.
    #[inline]
    fn storage_mut<T: Component>(&mut self) -> &mut Storage<T> {
        let tid = TypeId::of::<T>();
        if !self.storages.contains_key(&tid) {
            self.storages.insert(tid, Box::new(Storage::<T>::new()));
        }

        self.storages
            .get_mut(&tid)
            .and_then(|b| b.as_any_mut().downcast_mut::<Storage<T>>())
            .expect("storage type mismatch")
    }

    /// Returns immutable storage for `T` if it exists.
    #[inline]
    fn storage<T: Component>(&self) -> Option<&Storage<T>> {
        let tid = TypeId::of::<T>();
        self.storages
            .get(&tid)
            .and_then(|b| b.as_any().downcast_ref::<Storage<T>>())
    }

    /// Returns mutable storage for `T` if it exists (does not create it).
    #[inline]
    fn storage_mut_if_exists<T: Component>(&mut self) -> Option<&mut Storage<T>> {
        let tid = TypeId::of::<T>();
        self.storages
            .get_mut(&tid)
            .and_then(|b| b.as_any_mut().downcast_mut::<Storage<T>>())
    }

    /// Ensures component storage exists (no-op if already created).
    #[inline]
    pub fn ensure_storage<T: Component>(&mut self) {
        let _ = self.storage_mut::<T>();
    }

    /// Raw immutable access to the underlying component map.
    #[inline]
    pub fn components<T: Component>(&self) -> Option<&slotmap::SecondaryMap<EntityId, T>> {
        Some(&self.storage::<T>()?.map)
    }

    /// Raw mutable access to the underlying component map (does not create it).
    #[inline]
    pub fn components_mut<T: Component>(&mut self) -> Option<&mut slotmap::SecondaryMap<EntityId, T>> {
        Some(&mut self.storage_mut_if_exists::<T>()?.map)
    }

    /// Inserts (or replaces) a component on an entity.
    #[inline]
    pub fn insert<T: Component>(&mut self, id: EntityId, c: T) -> bool {
        if !self.exists(id) {
            return false;
        }
        self.storage_mut::<T>().map.insert(id, c);
        true
    }

    /// Removes a component from an entity (does not create storage).
    #[inline]
    pub fn remove<T: Component>(&mut self, id: EntityId) -> Option<T> {
        self.storage_mut_if_exists::<T>()?.map.remove(id)
    }

    #[inline]
    pub fn get<T: Component>(&self, id: EntityId) -> Option<&T> {
        self.storage::<T>()?.map.get(id)
    }

    /// Gets a mutable component reference (creates storage if missing).
    #[inline]
    pub fn get_mut<T: Component>(&mut self, id: EntityId) -> Option<&mut T> {
        self.storage_mut::<T>().map.get_mut(id)
    }

    #[inline]
    pub fn has<T: Component>(&self, id: EntityId) -> bool {
        self.get::<T>(id).is_some()
    }

    /// Zero-allocation query over entities that have component `T`.
    #[inline]
    pub fn query<T: Component>(&self) -> Query<'_, T> {
        Query {
            iter: self.storage::<T>().map(|s| s.map.iter()),
        }
    }

    /// Zero-allocation mutable query over entities that have component `T`.
    #[inline]
    pub fn query_mut<T: Component>(&mut self) -> QueryMut<'_, T> {
        QueryMut {
            iter: self.storage_mut_if_exists::<T>().map(|s| s.map.iter_mut()),
        }
    }

    /// Zero-allocation join query over entities that have both `A` and `B`.
    ///
    /// Internals: iterates the smaller component map and checks the other one.
    #[inline]
    pub fn query2<A: Component, B: Component>(&self) -> Query2<'_, A, B> {
        let a = self.storage::<A>().map(|s| &s.map);
        let b = self.storage::<B>().map(|s| &s.map);

        match (a, b) {
            (Some(am), Some(bm)) => {
                if am.len() <= bm.len() {
                    Query2::A(Query2A { iter: am.iter(), b: bm })
                } else {
                    Query2::B(Query2B { iter: bm.iter(), a: am })
                }
            }
            _ => Query2::Empty,
        }
    }

    /// Returns entity ids that have both `A` and `B`.
    ///
    /// Useful for safely performing staged updates on multiple component types.
    #[inline]
    pub fn query2_ids<A: Component, B: Component>(&self) -> impl Iterator<Item=EntityId> + '_ {
        self.query2::<A, B>().map(|(id, _, _)| id)
    }
}