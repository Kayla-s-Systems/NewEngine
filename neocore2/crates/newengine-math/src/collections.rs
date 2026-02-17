#![forbid(unsafe_op_in_unsafe_fn)]

//! Deterministic, engine-wide collection types.
//!
//! This module intentionally centralizes the engine's choice of hashing and generational-id
//! containers so that other crates don't proliferate ad-hoc dependencies and hashers.
//!
//! The public surface is intentionally small:
//! - `FxHashMap` / `FxHashSet` for fast, deterministic hashing
//! - slotmap types for stable generational keys
//! - re-exports of `hashbrown::hash_map` entry API

#[cfg(feature = "collections")]
use core::hash::BuildHasherDefault;

#[cfg(feature = "collections")]
pub use fxhash::FxHasher;

#[cfg(feature = "collections")]
pub type FxBuildHasher = BuildHasherDefault<FxHasher>;

#[cfg(feature = "collections")]
pub use hashbrown::{HashMap, HashSet};

/// Full `hashbrown` crate re-export for advanced APIs.
#[cfg(feature = "collections")]
pub use hashbrown as hashbrown;


#[cfg(feature = "collections")]
pub type FxHashMap<K, V> = HashMap<K, V, FxBuildHasher>;

#[cfg(feature = "collections")]
pub type FxHashSet<K> = HashSet<K, FxBuildHasher>;

/// Re-exported `Entry`/iter APIs.
#[cfg(feature = "collections")]
pub mod hash_map {
    pub use hashbrown::hash_map::*;
}

/// Slotmap types (stable generational keys).
#[cfg(feature = "collections")]
pub mod slot {
    pub use slotmap::{new_key_type, Key, SecondaryMap, SlotMap};
}

/// Full `slotmap` crate re-export for advanced APIs (e.g. `slotmap::secondary::Iter`).
#[cfg(feature = "collections")]
pub use slotmap as slotmap;

