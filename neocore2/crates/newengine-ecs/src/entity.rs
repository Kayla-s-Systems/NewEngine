#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_math::collections::prelude::ne_new_key_type;
use newengine_math::collections_prelude::NeKey;

ne_new_key_type! {
    pub struct EntityId;
}

impl EntityId {
    // NOTE: implements `NeKey` (slotmap key trait) via macro.

    /// Returns a deterministic, totally ordered representation of the entity id.
    ///
    /// This method intentionally hides the internal slotmap key layout from
    /// higher-level engine crates (Scene, Transform, etc).
    #[inline]
    pub fn stable_u64(self) -> u64 {
        self.data().as_ffi()
    }
}