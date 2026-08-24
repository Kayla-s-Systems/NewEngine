// Copyright (c) 2026 NewEngine | Take Some(). All rights reserved.
//! Engine-wide collections prelude (the normal API).
//!
//! # Contract
//! Downstream crates SHOULD import collections via this module (or the Ne* aliases)
//! and MUST NOT depend on raw implementation types (hashbrown::*, slotmap::*).
//!
//! - Internal/fast containers: `NeHashMap`, `NeHashSet`
//! - Untrusted/secure containers: `NeSecureHashMap`, `NeSecureHashSet`
//! - Bounded runtime cache: `NeBoundedCache`
//! - Constructors: `ne_hash_map()`, `ne_hash_set()`, `ne_*_with_capacity()`
//! - SlotMap aliases: `NeSlotMap`, `NeSecondaryMap`, `NeKey`, `ne_new_key_type`
//!
//! Do not call `HashMap::new()` / `HashSet::new()` through engine aliases.
//! Engine aliases use explicit hash policies, and hashbrown exposes `new()` only
//! for its default hasher. Use the constructors above or `Default::default()`.

use core::hash::{Hash, Hasher};

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

/// Engine bounded LRU-style runtime cache. Hits are O(1) average; eviction scans
/// only when capacity is exceeded.
pub type NeBoundedCache<K, V> = super::BoundedCache<K, V>;
pub use super::CacheStats as NeCacheStats;

#[inline]
pub fn ne_hash_map<K, V>() -> NeHashMap<K, V>
where
    K: Eq + Hash,
{
    NeHashMap::default()
}

#[inline]
pub fn ne_hash_set<T>() -> NeHashSet<T>
where
    T: Eq + Hash,
{
    NeHashSet::default()
}

#[inline]
pub fn ne_hash_map_with_capacity<K, V>(capacity: usize) -> NeHashMap<K, V>
where
    K: Eq + Hash,
{
    super::HashMap::with_capacity_and_hasher(capacity, NeFastBuildHasher::default())
}

#[inline]
pub fn ne_hash_set_with_capacity<T>(capacity: usize) -> NeHashSet<T>
where
    T: Eq + Hash,
{
    super::HashSet::with_capacity_and_hasher(capacity, NeFastBuildHasher::default())
}

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
// Queue / hash helpers
// ------------------------

pub type NeVecDeque<T> = super::VecDeque<T>;
pub type NeDefaultHasher = super::DefaultHasher;

#[inline]
pub fn ne_hash64<T: Hash + ?Sized>(value: &T) -> u64 {
    let mut h = NeDefaultHasher::new();
    value.hash(&mut h);
    h.finish()
}

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

#[inline]
pub fn ne_bounded_cache<K, V>(capacity: usize) -> NeBoundedCache<K, V>
where
    K: Eq + Hash + Clone,
{
    NeBoundedCache::new(capacity)
}
