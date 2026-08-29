use newengine_math::collections::prelude::{ne_new_key_type, NeKey};
use serde::{Deserialize, Serialize};

ne_new_key_type! {
    /// Stable, deterministic identifier of an entity across the engine.
    ///
    /// This low-level identity type intentionally lives in `newengine-entity-api`
    /// so API/contract crates can name entities without depending on the concrete
    /// ECS `World` implementation or a higher-level runtime crate.
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

/// Opaque service-safe entity handle.
///
/// The value is stable for diagnostics/tool calls but does not expose the native
/// key layout or allow consumers to manufacture a direct `EntityId`.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
pub struct EntityHandle {
    pub stable_id: u64,
}

impl EntityHandle {
    #[inline]
    pub const fn new(stable_id: u64) -> Self {
        Self { stable_id }
    }
}

impl From<EntityId> for EntityHandle {
    #[inline]
    fn from(value: EntityId) -> Self {
        Self::new(value.stable_u64())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handle_preserves_stable_id() {
        assert_eq!(EntityHandle::new(73).stable_id, 73);
    }
}
