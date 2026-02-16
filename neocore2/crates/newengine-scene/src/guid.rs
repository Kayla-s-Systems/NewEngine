#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_ecs::{EntityId, World};

use crate::components::EntityGuid;

/// Deterministic GUID allocator stored as a `World` resource.
///
/// Authoring/build pipelines require stable ids to keep diffs small and support
/// prefab overrides. The allocator is deterministic as long as entity creation
/// order is deterministic.
#[derive(Clone, Copy, Debug)]
pub struct GuidAllocator {
    pub seed: u64,
    pub next: u64,
}

impl Default for GuidAllocator {
    #[inline]
    fn default() -> Self {
        Self {
            // K-SYS / NewEngine: fixed seed for deterministic authoring.
            seed: 0x4B_53_59_53_4E_45_30_32,
            next: 1,
        }
    }
}

impl GuidAllocator {
    #[inline]
    pub fn alloc(&mut self) -> EntityGuid {
        let v = ((self.seed as u128) << 64) | (self.next as u128);
        self.next = self.next.wrapping_add(1).max(1);
        EntityGuid(v)
    }
}

/// Ensures the entity has an `EntityGuid`.
#[inline]
pub fn ensure_entity_guid(world: &mut World, id: EntityId) -> EntityGuid {
    if let Some(g) = world.get::<EntityGuid>(id) {
        return *g;
    }

    if world.resource::<GuidAllocator>().is_none() {
        world.insert_resource(GuidAllocator::default());
    }

    let g = world
        .resource_mut::<GuidAllocator>()
        .expect("allocator inserted")
        .alloc();
    let _ = world.insert(id, g);
    g
}
