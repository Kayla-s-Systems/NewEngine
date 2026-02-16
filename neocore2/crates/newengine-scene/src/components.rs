#![forbid(unsafe_op_in_unsafe_fn)]

use std::collections::BTreeMap;

/// Persistent entity identity for serialization, prefabs and cross-world references.
///
/// Runtime `EntityId` is not stable across loads; use `EntityGuid` for saved scenes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct EntityGuid(pub u128);

impl EntityGuid {
    #[inline]
    pub const fn as_u128(self) -> u128 {
        self.0
    }

    #[inline]
    pub const fn hi(self) -> u64 {
        (self.0 >> 64) as u64
    }

    #[inline]
    pub const fn lo(self) -> u64 {
        self.0 as u64
    }
}

/// Human-readable name of an entity.
#[derive(Clone, Debug)]
pub struct Name(pub String);

impl Name {
    #[inline]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

/// Marks the scene root.
#[derive(Clone, Copy, Debug, Default)]
pub struct SceneRoot;

/// Marks the active camera entity.
#[derive(Clone, Copy, Debug, Default)]
pub struct ActiveCamera;

/// Opaque gameplay/controller binding.
///
/// The engine core does not know what a controller "is"; concrete controllers live in gameplay
/// code (or plugins). This component only stores an identifier and optional state payload.
#[derive(Clone, Debug)]
pub struct Controller {
    /// Stable controller id (e.g. hash of a string or a plugin-provided id).
    pub kind: u64,
    /// Human-readable name for debugging/UI.
    pub kind_name: String,
    /// Opaque controller state owned by the controller implementation.
    pub state: Vec<u8>,
}

impl Controller {
    #[inline]
    pub fn new(kind: u64, kind_name: impl Into<String>, state: Vec<u8>) -> Self {
        Self {
            kind,
            kind_name: kind_name.into(),
            state,
        }
    }
}

/// Generic, editor-friendly property bag.
///
/// For gameplay performance prefer typed components (e.g. `Health`, `Armor`) instead of using a
/// string-keyed map. This type exists for scripting, UI inspection and prototyping.
#[derive(Clone, Debug, Default)]
pub struct PropertyBag {
    pub props: BTreeMap<String, PropertyValue>,
}

#[derive(Clone, Debug)]
pub enum PropertyValue {
    Bool(bool),
    I64(i64),
    U64(u64),
    F32(f32),
    F64(f64),
    String(String),
    Bytes(Vec<u8>),
    Vec3([f32; 3]),
}