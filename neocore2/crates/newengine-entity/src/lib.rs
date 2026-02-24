#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_math::collections::prelude::{ne_new_key_type, NeKey};

ne_new_key_type! {
    /// Stable, deterministic identifier of an entity across the engine.
    ///
    /// This type is intentionally defined outside of ECS storage so that higher-level crates
    /// (Scene, Transform, Camera, Editor) depend on the *concept of an entity*, not on a
    /// particular ECS implementation.
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

/// Entity kind is a stable, engine-wide category tag.
///
/// The core crate intentionally provides only the type; project-specific meaning should live in
/// plugins/tools to avoid hard-coded enums in the engine host.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
#[repr(transparent)]
pub struct EntityKind(pub u32);

impl EntityKind {
    #[inline]
    pub const fn new(v: u32) -> Self {
        Self(v)
    }

    #[inline]
    pub const fn value(self) -> u32 {
        self.0
    }
}

/// Non-authoritative editor/runtime metadata attached to an entity.
///
/// This is not used for simulation logic. It exists to support tools and UX (inspector, search,
/// serialization of authoring data).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EntityMeta {
    pub kind: EntityKind,
    pub name: EntityName,
    pub flags: u32,
}

impl Default for EntityMeta {
    #[inline]
    fn default() -> Self {
        Self {
            kind: EntityKind::new(0),
            name: EntityName::new(""),
            flags: 0,
        }
    }
}

/// Human-readable name for tools.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct EntityName(String);

impl EntityName {
    #[inline]
    pub fn new(s: impl AsRef<str>) -> Self {
        Self(s.as_ref().to_owned())
    }

    #[inline]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
