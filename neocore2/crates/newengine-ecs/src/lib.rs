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

/// A small, deterministic ECS world.
///
/// - Entities are generational keys.
/// - Components are stored in typed `SecondaryMap`s.
/// - Component access is type-safe.
///
/// This is intentionally minimal and editor-friendly; the higher-level systems
/// (transform propagation, rendering extraction, etc.) live in separate crates.
pub struct World {
    entities: SlotMap<EntityId, ()>,
    storages: HashMap<TypeId, Box<dyn ErasedStorage>>,
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

    #[inline]
    fn storage<T: 'static>(&self) -> Option<&Storage<T>> {
        let tid = TypeId::of::<T>();
        self.storages
            .get(&tid)
            .and_then(|b| b.as_any().downcast_ref::<Storage<T>>())
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

    #[inline]
    pub fn remove<T: 'static>(&mut self, id: EntityId) -> Option<T> {
        self.storage_mut::<T>().map.remove(id)
    }

    #[inline]
    pub fn get<T: 'static>(&self, id: EntityId) -> Option<&T> {
        self.storage::<T>()?.map.get(id)
    }

    #[inline]
    pub fn get_mut<T: 'static>(&mut self, id: EntityId) -> Option<&mut T> {
        self.storage_mut::<T>().map.get_mut(id)
    }

    /// Iterates entities that have a component `T`.
    #[inline]
    pub fn iter_with<T: 'static>(&self) -> impl Iterator<Item=(EntityId, &T)> + '_ {
        self.storage::<T>()
            .map(|s| s.map.iter().map(|(id, c)| (id, c)).collect::<Vec<_>>())
            .unwrap_or_default()
            .into_iter()
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
