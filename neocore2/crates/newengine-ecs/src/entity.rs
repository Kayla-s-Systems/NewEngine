#![forbid(unsafe_op_in_unsafe_fn)]

use slotmap::{new_key_type, Key};

new_key_type! {
    pub struct EntityId;
}

impl EntityId {
    /// Returns a deterministic, totally ordered representation of the entity id.
    ///
    /// This method intentionally hides the internal slotmap key layout from
    /// higher-level engine crates (Scene, Transform, etc).
    #[inline]
    pub fn stable_u64(self) -> u64 {
        self.data().as_ffi() as u64
    }
}