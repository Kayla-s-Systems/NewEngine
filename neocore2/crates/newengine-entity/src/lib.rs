#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_math::collections::prelude::ne_new_key_type;
use newengine_math::collections_prelude::NeKey;

ne_new_key_type! {
    /// Stable, deterministic identifier of an entity across the engine.
    ///
    /// This type is intentionally defined outside of ECS storage so that higher-level
    /// crates (Scene, Transform, Camera, Editor) depend on the concept of an entity,
    /// not on a particular ECS implementation.
    pub struct EntityId;
}

impl EntityId {
    /// Returns a deterministic, totally ordered representation of the entity id.
    ///
    /// This method intentionally hides the internal generational key layout.
    #[inline]
    pub fn stable_u64(self) -> u64 {
        self.data().as_ffi()
    }
}
