use std::collections::BTreeMap;

use newengine_ecs::EntityId;
use newengine_physics_contracts::PhysicsHandle;

#[derive(Default)]
pub struct JoltBodyMap {
    by_entity: BTreeMap<u64, PhysicsHandle>,
}

impl JoltBodyMap {
    #[inline]
    pub fn insert(&mut self, entity: EntityId, handle: PhysicsHandle) {
        self.by_entity.insert(entity.stable_u64(), handle);
    }

    #[inline]
    pub fn get(&self, entity: EntityId) -> Option<PhysicsHandle> {
        self.by_entity.get(&entity.stable_u64()).copied()
    }

    #[inline]
    pub fn remove(&mut self, entity: EntityId) -> Option<PhysicsHandle> {
        self.by_entity.remove(&entity.stable_u64())
    }
}
