// Copyright (c) 2026 NewEngine | Take Some(). All rights reserved.
#![forbid(unsafe_op_in_unsafe_fn)]

use core::borrow::Borrow;
use core::hash::Hash;

use super::FxHashMap;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
}

#[derive(Clone, Debug)]
struct CacheEntry<V> {
    value: V,
    last_used: u64,
}

/// Small/medium bounded runtime cache with O(1) average lookup and LRU eviction.
///
/// The hot path never mutates an ordered queue. Recency is represented by a
/// monotonic stamp and the oldest key is scanned only when an insertion must
/// evict an entry. This is intentionally optimized for engine caches where hits
/// vastly outnumber capacity overflows.
#[derive(Clone, Debug)]
pub struct BoundedCache<K, V> {
    entries: FxHashMap<K, CacheEntry<V>>,
    capacity: usize,
    clock: u64,
    stats: CacheStats,
}

impl<K, V> BoundedCache<K, V>
where
    K: Eq + Hash + Clone,
{
    #[inline]
    pub fn new(capacity: usize) -> Self {
        let capacity = capacity.max(1);
        Self {
            entries: FxHashMap::with_capacity_and_hasher(capacity, Default::default()),
            capacity,
            clock: 0,
            stats: CacheStats::default(),
        }
    }

    #[inline]
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    #[inline]
    pub fn stats(&self) -> CacheStats {
        self.stats
    }

    #[inline]
    pub fn reset_stats(&mut self) {
        self.stats = CacheStats::default();
    }

    #[inline]
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    #[inline]
    pub fn contains_key<Q>(&self, key: &Q) -> bool
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self.entries.contains_key(key)
    }

    #[inline]
    pub fn peek<Q>(&self, key: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self.entries.get(key).map(|entry| &entry.value)
    }

    pub fn get<Q>(&mut self, key: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self.clock = self.clock.saturating_add(1);
        let stamp = self.clock;
        match self.entries.get_mut(key) {
            Some(entry) => {
                entry.last_used = stamp;
                self.stats.hits = self.stats.hits.saturating_add(1);
                Some(&entry.value)
            }
            None => {
                self.stats.misses = self.stats.misses.saturating_add(1);
                None
            }
        }
    }

    pub fn get_mut<Q>(&mut self, key: &Q) -> Option<&mut V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self.clock = self.clock.saturating_add(1);
        let stamp = self.clock;
        match self.entries.get_mut(key) {
            Some(entry) => {
                entry.last_used = stamp;
                self.stats.hits = self.stats.hits.saturating_add(1);
                Some(&mut entry.value)
            }
            None => {
                self.stats.misses = self.stats.misses.saturating_add(1);
                None
            }
        }
    }

    pub fn insert(&mut self, key: K, value: V) -> Option<(K, V)> {
        self.clock = self.clock.saturating_add(1);
        let stamp = self.clock;

        if let Some(entry) = self.entries.get_mut(&key) {
            entry.value = value;
            entry.last_used = stamp;
            return None;
        }

        let evicted = if self.entries.len() >= self.capacity {
            let oldest = self
                .entries
                .iter()
                .min_by_key(|(_, entry)| entry.last_used)
                .map(|(key, _)| key.clone());
            oldest.and_then(|oldest| {
                self.entries.remove(&oldest).map(|entry| {
                    self.stats.evictions = self.stats.evictions.saturating_add(1);
                    (oldest, entry.value)
                })
            })
        } else {
            None
        };

        self.entries.insert(
            key,
            CacheEntry {
                value,
                last_used: stamp,
            },
        );
        evicted
    }

    #[inline]
    pub fn remove<Q>(&mut self, key: &Q) -> Option<V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self.entries.remove(key).map(|entry| entry.value)
    }

    pub fn retain(&mut self, mut keep: impl FnMut(&K, &mut V) -> bool) {
        self.entries
            .retain(|key, entry| keep(key, &mut entry.value));
    }

    pub fn most_recent_key(&self) -> Option<&K> {
        self.entries
            .iter()
            .max_by_key(|(_, entry)| entry.last_used)
            .map(|(key, _)| key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hit_promotes_entry_and_lru_evicts_oldest() {
        let mut cache = BoundedCache::new(2);
        cache.insert("a".to_owned(), 1);
        cache.insert("b".to_owned(), 2);
        assert_eq!(cache.get("a"), Some(&1));
        let evicted = cache.insert("c".to_owned(), 3);
        assert_eq!(evicted, Some(("b".to_owned(), 2)));
        assert!(cache.contains_key("a"));
        assert!(cache.contains_key("c"));
        assert!(!cache.contains_key("b"));
        assert_eq!(cache.stats().evictions, 1);
    }

    #[test]
    fn stats_track_hits_and_misses_without_counting_peek() {
        let mut cache = BoundedCache::new(2);
        cache.insert("a".to_owned(), 7);
        assert_eq!(cache.peek("a"), Some(&7));
        assert_eq!(cache.get("a"), Some(&7));
        assert_eq!(cache.get("missing"), None);
        assert_eq!(
            cache.stats(),
            CacheStats {
                hits: 1,
                misses: 1,
                evictions: 0,
            }
        );
    }
}
