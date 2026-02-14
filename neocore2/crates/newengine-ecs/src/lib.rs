#![forbid(unsafe_op_in_unsafe_fn)]

use core::any::{Any, TypeId};
use hashbrown::HashMap;
use slotmap::{new_key_type, SecondaryMap, SlotMap};

new_key_type! {
    /// Generational entity identifier.
    pub struct EntityId;
}

trait ErasedStorage {
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
    fn remove_entity(&mut self, id: EntityId);
}

struct Storage<T: 'static> {
    map: SecondaryMap<EntityId, T>,
}

impl<T: 'static> Storage<T> {
    #[inline]
    fn new() -> Self {
        Self {
            map: SecondaryMap::new(),
        }
    }
}

impl<T: 'static> ErasedStorage for Storage<T> {
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
pub struct Query<'a, T: 'static> {
    iter: Option<slotmap::secondary::Iter<'a, EntityId, T>>,
}

impl<'a, T: 'static> Iterator for Query<'a, T> {
    type Item = (EntityId, &'a T);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.iter.as_mut()?.next()
    }
}

/// Mutable query iterator over a single component type.
pub struct QueryMut<'a, T: 'static> {
    iter: Option<slotmap::secondary::IterMut<'a, EntityId, T>>,
}

impl<'a, T: 'static> Iterator for QueryMut<'a, T> {
    type Item = (EntityId, &'a mut T);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.iter.as_mut()?.next()
    }
}

/// A small, deterministic ECS world.
///
/// Design goals:
/// - deterministic entity identity via generational keys
/// - type-safe component storage
/// - no hidden allocations on iteration
/// - editor-friendly (command/deferred patterns live above ECS)
pub struct World {
    entities: SlotMap<EntityId, ()>,
    storages: HashMap<TypeId, Box<dyn ErasedStorage>>,
    resources: HashMap<TypeId, Box<dyn Any>>,
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
    pub fn insert_resource<T: 'static>(&mut self, r: T) {
        self.resources.insert(TypeId::of::<T>(), Box::new(r));
    }

    /// Returns an immutable resource reference.
    #[inline]
    pub fn resource<T: 'static>(&self) -> Option<&T> {
        self.resources
            .get(&TypeId::of::<T>())
            .and_then(|b| b.downcast_ref::<T>())
    }

    /// Returns a mutable resource reference.
    #[inline]
    pub fn resource_mut<T: 'static>(&mut self) -> Option<&mut T> {
        self.resources
            .get_mut(&TypeId::of::<T>())
            .and_then(|b| b.downcast_mut::<T>())
    }

    /// Removes a resource.
    #[inline]
    pub fn remove_resource<T: 'static>(&mut self) -> Option<T> {
        self.resources
            .remove(&TypeId::of::<T>())
            .and_then(|b| b.downcast::<T>().ok())
            .map(|b| *b)
    }

    /// Ensures storage for component T exists and returns mutable access to it.
    #[inline]
    fn storage_mut<T: 'static>(&mut self) -> &mut Storage<T> {
        let tid = TypeId::of::<T>();
        if !self.storages.contains_key(&tid) {
            self.storages.insert(tid, Box::new(Storage::<T>::new()));
        }

        self.storages
            .get_mut(&tid)
            .and_then(|b| b.as_any_mut().downcast_mut::<Storage<T>>())
            .expect("storage type mismatch")
    }

    /// Returns immutable storage for T if it exists.
    #[inline]
    fn storage<T: 'static>(&self) -> Option<&Storage<T>> {
        let tid = TypeId::of::<T>();
        self.storages
            .get(&tid)
            .and_then(|b| b.as_any().downcast_ref::<Storage<T>>())
    }

    /// Returns mutable storage for T if it exists (does not create it).
    #[inline]
    fn storage_mut_if_exists<T: 'static>(&mut self) -> Option<&mut Storage<T>> {
        let tid = TypeId::of::<T>();
        self.storages
            .get_mut(&tid)
            .and_then(|b| b.as_any_mut().downcast_mut::<Storage<T>>())
    }

    /// Ensures component storage exists (no-op if already created).
    #[inline]
    pub fn ensure_storage<T: 'static>(&mut self) {
        let _ = self.storage_mut::<T>();
    }

    /// Raw immutable access to the underlying component map.
    #[inline]
    pub fn components<T: 'static>(&self) -> Option<&SecondaryMap<EntityId, T>> {
        Some(&self.storage::<T>()?.map)
    }

    /// Raw mutable access to the underlying component map (does not create it).
    #[inline]
    pub fn components_mut<T: 'static>(&mut self) -> Option<&mut SecondaryMap<EntityId, T>> {
        Some(&mut self.storage_mut_if_exists::<T>()?.map)
    }

    /// Inserts (or replaces) a component on an entity.
    #[inline]
    pub fn insert<T: 'static>(&mut self, id: EntityId, c: T) -> bool {
        if !self.exists(id) {
            return false;
        }
        self.storage_mut::<T>().map.insert(id, c);
        true
    }

    /// Removes a component from an entity (does not create storage).
    #[inline]
    pub fn remove<T: 'static>(&mut self, id: EntityId) -> Option<T> {
        self.storage_mut_if_exists::<T>()?.map.remove(id)
    }

    #[inline]
    pub fn get<T: 'static>(&self, id: EntityId) -> Option<&T> {
        self.storage::<T>()?.map.get(id)
    }

    /// Gets a mutable component reference (creates storage if missing).
    ///
    /// Note: if you want "no create" semantics, use `components_mut()` and look up in the map.
    #[inline]
    pub fn get_mut<T: 'static>(&mut self, id: EntityId) -> Option<&mut T> {
        self.storage_mut::<T>().map.get_mut(id)
    }

    #[inline]
    pub fn has<T: 'static>(&self, id: EntityId) -> bool {
        self.get::<T>(id).is_some()
    }

    /// Zero-allocation query over entities that have component T.
    #[inline]
    pub fn query<T: 'static>(&self) -> Query<'_, T> {
        Query {
            iter: self.storage::<T>().map(|s| s.map.iter()),
        }
    }

    /// Zero-allocation mutable query over entities that have component T.
    #[inline]
    pub fn query_mut<T: 'static>(&mut self) -> QueryMut<'_, T> {
        QueryMut {
            iter: self.storage_mut_if_exists::<T>().map(|s| s.map.iter_mut()),
        }
    }

    /// Zero-allocation join query over entities that have both `A` and `B`.
    ///
    /// Internals: iterates the smaller component map and checks the other one.
    #[inline]
    pub fn query2<A: 'static, B: 'static>(&self) -> Query2<'_, A, B> {
        let a = self.storage::<A>().map(|s| &s.map);
        let b = self.storage::<B>().map(|s| &s.map);

        match (a, b) {
            (Some(am), Some(bm)) => {
                if am.len() <= bm.len() {
                    Query2::A(Query2A {
                        iter: am.iter(),
                        b: bm,
                    })
                } else {
                    Query2::B(Query2B {
                        iter: bm.iter(),
                        a: am,
                    })
                }
            }
            _ => Query2::Empty,
        }
    }

    /// Returns entity ids that have both `A` and `B`.
    ///
    /// This is useful for safely performing mutable updates on multiple component types.
    #[inline]
    pub fn query2_ids<A: 'static, B: 'static>(&self) -> impl Iterator<Item=EntityId> + '_ {
        self.query2::<A, B>().map(|(id, _, _)| id)
    }
}

/// Join query iterator over two component types.
pub enum Query2<'a, A: 'static, B: 'static> {
    Empty,
    A(Query2A<'a, A, B>),
    B(Query2B<'a, A, B>),
}

pub struct Query2A<'a, A: 'static, B: 'static> {
    iter: slotmap::secondary::Iter<'a, EntityId, A>,
    b: &'a SecondaryMap<EntityId, B>,
}

pub struct Query2B<'a, A: 'static, B: 'static> {
    iter: slotmap::secondary::Iter<'a, EntityId, B>,
    a: &'a SecondaryMap<EntityId, A>,
}

impl<'a, A: 'static, B: 'static> Iterator for Query2<'a, A, B> {
    type Item = (EntityId, &'a A, &'a B);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Query2::Empty => None,
            Query2::A(q) => {
                while let Some((id, a)) = q.iter.next() {
                    if let Some(b) = q.b.get(id) {
                        return Some((id, a, b));
                    }
                }
                None
            }
            Query2::B(q) => {
                while let Some((id, b)) = q.iter.next() {
                    if let Some(a) = q.a.get(id) {
                        return Some((id, a, b));
                    }
                }
                None
            }
        }
    }
}

/// High-level editor operations are expressed as commands to enable undo/redo later.
pub enum Command {
    Spawn,
    Despawn { entity: EntityId },
    InsertName { entity: EntityId, name: String },
}

/// Command application for MVP; undo/redo will be built on top later.
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
        std::mem::take(&mut self.queue).into_iter()
    }
}