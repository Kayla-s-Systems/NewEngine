#![forbid(unsafe_op_in_unsafe_fn)]

//! Engine-wide collection types and policies.
//!
//! This module centralizes container + hashing choices to avoid ad-hoc dependencies across crates.
//!
//! ## Determinism notes
//! - HashMap/HashSet iteration order is NOT a determinism guarantee.
//! - For stable iteration/serialization, use `BTreeMap/BTreeSet` or sort keys explicitly.
//! - SlotMap keys are runtime handles; do not serialize them as stable IDs.
//!
//! ## API surface
//! - Normal usage: `use newengine_math::collections::prelude::*;`
//! - Escape hatch: `use newengine_math::collections::raw::...;` (implementation-specific APIs)

#[cfg(feature = "collections")]
use core::hash::BuildHasherDefault;

#[cfg(feature = "collections")]
pub mod policy;

#[cfg(feature = "collections")]
pub mod prelude;

#[cfg(feature = "collections")]
pub mod raw;

#[cfg(feature = "collections")]
pub use fxhash::FxHasher;

#[cfg(feature = "collections")]
pub type FxBuildHasher = BuildHasherDefault<FxHasher>;

#[cfg(feature = "collections")]
pub use hashbrown::{HashMap, HashSet};

/// Fast, deterministic hash map for *internal engine data*.
///
/// ⚠️ Not DoS-resistant. Do not use for untrusted input.
#[cfg(feature = "collections")]
pub type FxHashMap<K, V> = HashMap<K, V, FxBuildHasher>;

/// Fast, deterministic hash set for *internal engine data*.
///
/// ⚠️ Not DoS-resistant. Do not use for untrusted input.
#[cfg(feature = "collections")]
pub type FxHashSet<K> = HashSet<K, FxBuildHasher>;

/// Secure, randomized hashing for untrusted inputs (network/JSON/modding).
///
/// This is not deterministic between runs by design.
#[cfg(feature = "collections")]
pub type SecureBuildHasher = std::collections::hash_map::RandomState;

/// Hash map for untrusted/external inputs (secure).
#[cfg(feature = "collections")]
pub type SecureHashMap<K, V> = HashMap<K, V, SecureBuildHasher>;

/// Hash set for untrusted/external inputs (secure).
#[cfg(feature = "collections")]
pub type SecureHashSet<K> = HashSet<K, SecureBuildHasher>;

/// Stable-iteration containers.
#[cfg(feature = "collections")]
pub type BTreeMap<K, V> = std::collections::BTreeMap<K, V>;

#[cfg(feature = "collections")]
pub type BTreeSet<K> = std::collections::BTreeSet<K>;

/// Slotmap types (stable generational keys).
///
/// These are re-exported only for wiring. Prefer `collections::prelude::*` in downstream crates.
#[cfg(feature = "collections")]
pub use slotmap;

