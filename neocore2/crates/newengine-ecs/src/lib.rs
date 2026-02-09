#![forbid(unsafe_op_in_unsafe_fn)]

use core::any::{Any, TypeId};

use hashbrown::HashMap;
use slotmap::{new_key_type, SecondaryMap, SlotMap};

new_key_type! {
    /// Generational entity identifier.
    pub struct EntityId;
}

trait ErasedStorage: Send + Sync {
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
    fn remove_entity(&mut self, id: EntityId);
}

struct Storage<T: Send + Sync + 'static> {
    map: SecondaryMap<EntityId, T>,
}

impl<T: Send + Sync + 'static> Storage<T> {
    #[inline]
    fn new() -> Self {
        Self {
            map: SecondaryMap::new(),
        }
    }
}

impl<T: Send + Sync + 'static> ErasedStorage for Storage<T> {
    #[inline]
    fn as_any(&self) -> &dyn Any {
        self
    }

    #[inline]
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    #[inline]
    fn remove_entity(&mut self, id: EntityId) {
        self.map.remove(id);
    }
}

/// Immutable query iterator over a single component type.
pub struct Query<'a, T: Send + Sync + 'static> {
    iter: Option<slotmap::secondary::Iter<'a, EntityId, T>>,
}

impl<'a, T: Send + Sync + 'static> Iterator for Query<'a, T> {
    type Item = (EntityId, &'a T);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.iter.as_mut()?.next()
    }
}

/// Mutable query iterator over a single component type.
pub struct QueryMut<'a, T: Send + Sync + 'static> {
    iter: Option<slotmap::secondary::IterMut<'a, EntityId, T>>,
}

impl<'a, T: Send + Sync + 'static> Iterator for QueryMut<'a, T> {
    type Item = (EntityId, &'a mut T);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.iter.as_mut()?.next()
    }
}

/// Immutable query iterator for two component types.
pub struct Query2<'a, A: Send + Sync + 'static, B: Send + Sync + 'static> {
    iter_a: slotmap::secondary::Iter<'a, EntityId, A>,
    map_b: &'a SecondaryMap<EntityId, B>,
}

impl<'a, A: Send + Sync + 'static, B: Send + Sync + 'static> Iterator for Query2<'a, A, B> {
    type Item = (EntityId, &'a A, &'a B);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let (id, a) = self.iter_a.next()?;
            if let Some(b) = self.map_b.get(id) {
                return Some((id, a, b));
            }
        }
    }
}

/// A small, deterministic ECS world.
///
/// Threading: `World` is `Send + Sync` (intended to be shared behind locks).
/// Therefore component and resource types must be `Send + Sync + 'static`.
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

    #[inline]
    fn storage_mut<T: Send + Sync + 'static>(&mut self) -> &mut Storage<T> {
        let tid = TypeId::of::<T>();
        if !self.storages.contains_key(&tid) {
            self.storages.insert(tid, Box::new(Storage::<T>::new()));
        }

        self.storages
            .get_mut(&tid)
            .and_then(|b| b.as_any_mut().downcast_mut::<Storage<T>>())
            .expect("storage type mismatch")
    }

    #[inline]
    fn storage<T: Send + Sync + 'static>(&self) -> Option<&Storage<T>> {
        let tid = TypeId::of::<T>();
        self.storages
            .get(&tid)
            .and_then(|b| b.as_any().downcast_ref::<Storage<T>>())
    }

    #[inline]
    fn storage_mut_if_exists<T: Send + Sync + 'static>(&mut self) -> Option<&mut Storage<T>> {
        let tid = TypeId::of::<T>();
        self.storages
            .get_mut(&tid)
            .and_then(|b| b.as_any_mut().downcast_mut::<Storage<T>>())
    }

    #[inline]
    pub fn ensure_storage<T: Send + Sync + 'static>(&mut self) {
        let _ = self.storage_mut::<T>();
    }

    /// Raw immutable access to a component map.
    #[inline]
    pub fn components<T: Send + Sync + 'static>(&self) -> Option<&SecondaryMap<EntityId, T>> {
        Some(&self.storage::<T>()?.map)
    }

    /// Raw mutable access to a component map (does not create storage).
    #[inline]
    pub fn components_mut<T: Send + Sync + 'static>(&mut self) -> Option<&mut SecondaryMap<EntityId, T>> {
        Some(&mut self.storage_mut_if_exists::<T>()?.map)
    }

    /// Inserts (or replaces) a component on an entity.
    #[inline]
    pub fn insert<T: Send + Sync + 'static>(&mut self, id: EntityId, c: T) -> bool {
        if !self.exists(id) {
            return false;
        }
        self.storage_mut::<T>().map.insert(id, c);
        true
    }

    /// Removes a component from an entity (does not create storage).
    #[inline]
    pub fn remove<T: Send + Sync + 'static>(&mut self, id: EntityId) -> Option<T> {
        self.storage_mut_if_exists::<T>()?.map.remove(id)
    }

    #[inline]
    pub fn get<T: Send + Sync + 'static>(&self, id: EntityId) -> Option<&T> {
        self.storage::<T>()?.map.get(id)
    }

    #[inline]
    pub fn get_mut<T: Send + Sync + 'static>(&mut self, id: EntityId) -> Option<&mut T> {
        self.storage_mut::<T>().map.get_mut(id)
    }

    #[inline]
    pub fn has<T: Send + Sync + 'static>(&self, id: EntityId) -> bool {
        self.get::<T>(id).is_some()
    }

    /// Zero-allocation query over entities that have component `T`.
    #[inline]
    pub fn query<T: Send + Sync + 'static>(&self) -> Query<'_, T> {
        Query {
            iter: self.storage::<T>().map(|s| s.map.iter()),
        }
    }

    /// Zero-allocation mutable query over entities that have component `T`.
    #[inline]
    pub fn query_mut<T: Send + Sync + 'static>(&mut self) -> QueryMut<'_, T> {
        QueryMut {
            iter: self.storage_mut_if_exists::<T>().map(|s| s.map.iter_mut()),
        }
    }

    /// Zero-allocation query over entities that have both `A` and `B`.
    #[inline]
    pub fn query2<A: Send + Sync + 'static, B: Send + Sync + 'static>(&self) -> Option<Query2<'_, A, B>> {
        let a = self.storage::<A>()?;
        let b = self.storage::<B>()?;
        Some(Query2 {
            iter_a: a.map.iter(),
            map_b: &b.map,
        })
    }

    /// Returns entity ids that have both `A` and `B`.
    ///
    /// This is designed for safe multi-pass: collect ids, then mutate per-entity.
    #[inline]
    pub fn query2_ids<A: Send + Sync + 'static, B: Send + Sync + 'static>(&self) -> Vec<EntityId> {
        let Some(a) = self.storage::<A>() else { return Vec::new(); };
        let Some(b) = self.storage::<B>() else { return Vec::new(); };
        a.map.keys().filter(|&id| b.map.contains_key(id)).collect()
    }

    /// Resources (singletons)
    #[inline]
    pub fn insert_resource<R: Send + Sync + 'static>(&mut self, r: R) {
        self.resources.insert(TypeId::of::<R>(), Box::new(r));
    }

    #[inline]
    pub fn resource<R: Send + Sync + 'static>(&self) -> Option<&R> {
        self.resources
            .get(&TypeId::of::<R>())
            .and_then(|b| b.downcast_ref::<R>())
    }

    #[inline]
    pub fn resource_mut<R: Send + Sync + 'static>(&mut self) -> Option<&mut R> {
        self.resources
            .get_mut(&TypeId::of::<R>())
            .and_then(|b| b.downcast_mut::<R>())
    }

    #[inline]
    pub fn remove_resource<R: Send + Sync + 'static>(&mut self) -> Option<R> {
        let b = self.resources.remove(&TypeId::of::<R>())?;
        b.downcast::<R>().ok().map(|b| *b)
    }
}