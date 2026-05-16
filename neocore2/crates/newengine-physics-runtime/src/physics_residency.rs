use std::collections::BTreeSet;

use newengine_ecs::EntityId;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PhysicsResidencyCommand {
    Prepare(EntityId),
    Commit(EntityId),
    Evict(EntityId),
}

#[derive(Clone, Debug, Default)]
pub struct PhysicsResidencySet {
    prepared: BTreeSet<u64>,
    resident: BTreeSet<u64>,
}

impl PhysicsResidencySet {
    #[inline]
    pub fn apply(&mut self, command: PhysicsResidencyCommand) {
        match command {
            PhysicsResidencyCommand::Prepare(id) => { self.prepared.insert(id.stable_u64()); }
            PhysicsResidencyCommand::Commit(id) => {
                let key = id.stable_u64();
                self.prepared.remove(&key);
                self.resident.insert(key);
            }
            PhysicsResidencyCommand::Evict(id) => {
                let key = id.stable_u64();
                self.prepared.remove(&key);
                self.resident.remove(&key);
            }
        }
    }

    #[inline]
    pub fn is_resident(&self, id: EntityId) -> bool { self.resident.contains(&id.stable_u64()) }
}
