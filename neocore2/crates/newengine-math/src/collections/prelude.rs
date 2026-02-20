// Copyright (c) 2026 NewEngine | Kayla's Systems. All rights reserved.
//! Engine-wide collections prelude (the normal API).
//!
//! # Contract
//! Downstream crates SHOULD import collections via this module (or the Ne* aliases)
//! and MUST NOT depend on raw implementation types (hashbrown::*, slotmap::*).
//!
//! - Internal/fast containers: `NeHashMap`, `NeHashSet`
//! - Untrusted/secure containers: `NeSecureHashMap`, `NeSecureHashSet`
//! - SlotMap aliases: `NeSlotMap`, `NeSecondaryMap`, `NeKey`, `ne_new_key_type`

use core::hash::Hash;

use super::{FxHashMap, FxHashSet};

/// Engine-default fast/internal hasher builder.
pub type NeFastBuildHasher = super::policy::FastBuildHasher;

/// Engine-default secure/untrusted hasher builder.
pub type NeSecureBuildHasher = super::policy::SecureBuildHasher;

// ------------------------
// Hash maps / sets (FAST)
// ------------------------

/// Engine hash map for *internal* data (fast, deterministic).
pub type NeHashMap<K, V> = FxHashMap<K, V>;

/// Engine hash set for *internal* data (fast, deterministic).
pub type NeHashSet<T> = FxHashSet<T>;

// --------------------------
// Hash maps / sets (SECURE)
// --------------------------

/// Engine hash map for *untrusted* data (secure, randomized).
pub type NeSecureHashMap<K, V> = super::HashMap<K, V, NeSecureBuildHasher>;

/// Engine hash set for *untrusted* data (secure, randomized).
pub type NeSecureHashSet<T> = super::HashSet<T, NeSecureBuildHasher>;

// Convenience aliases for readability at call sites.
pub type InternalMap<K, V> = NeHashMap<K, V>;
pub type InternalSet<T> = NeHashSet<T>;
pub type UntrustedMap<K, V> = NeSecureHashMap<K, V>;
pub type UntrustedSet<T> = NeSecureHashSet<T>;

// ------------------------
// Stable order containers
// ------------------------

pub type NeBTreeMap<K, V> = super::BTreeMap<K, V>;
pub type NeBTreeSet<K> = super::BTreeSet<K>;

// ------------------------
// SlotMap family
// ------------------------

pub type NeSlotMap<K, V> = super::slotmap::SlotMap<K, V>;
pub type NeSecondaryMap<K, V> = super::slotmap::SecondaryMap<K, V>;
pub use super::slotmap::new_key_type as ne_new_key_type;
pub use super::slotmap::Key as NeKey;

// ------------------------
// Explicit constructors
// ------------------------

#[inline]
pub fn ne_untrusted_map<K, V>() -> UntrustedMap<K, V>
where
    K: Eq + Hash,
{
    UntrustedMap::with_hasher(NeSecureBuildHasher::default())
}

#[inline]
pub fn ne_untrusted_set<T>() -> UntrustedSet<T>
where
    T: Eq + Hash,
{
    UntrustedSet::with_hasher(NeSecureBuildHasher::default())
}
