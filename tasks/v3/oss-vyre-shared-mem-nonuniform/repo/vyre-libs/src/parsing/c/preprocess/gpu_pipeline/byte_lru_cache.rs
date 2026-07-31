//! Byte-bounded LRU cache core for GPU preprocessor resident caches.

use std::hash::Hash;

use rustc_hash::FxHashMap as HashMap;

use super::lru_index::LruIndex;

#[derive(Copy, Clone)]
pub(super) struct ByteLruPanicLabels {
    pub(super) byte_add_overflow: &'static str,
    pub(super) byte_sub_underflow: &'static str,
    pub(super) epoch_overflow: &'static str,
}

pub(super) struct ByteBoundLruCache<K, V> {
    entries: HashMap<K, ByteLruEntry<V>>,
    bytes: usize,
    max_entries: usize,
    max_bytes: usize,
    epoch: u64,
    lru: LruIndex<K>,
    labels: ByteLruPanicLabels,
}

struct ByteLruEntry<V> {
    value: V,
    bytes: usize,
    last_access: u64,
}

impl<K, V> ByteBoundLruCache<K, V>
where
    K: Clone + Eq + Hash,
{
    pub(super) fn new(max_entries: usize, max_bytes: usize, labels: ByteLruPanicLabels) -> Self {
        Self {
            entries: HashMap::default(),
            bytes: 0,
            max_entries,
            max_bytes,
            epoch: 0,
            lru: LruIndex::with_capacity(max_entries),
            labels,
        }
    }

    pub(super) fn lookup_cloned(&mut self, key: &K) -> Option<V>
    where
        V: Clone,
    {
        let next_epoch = self.next_epoch();
        let entry = self.entries.get_mut(key)?;
        entry.last_access = next_epoch;
        let value = entry.value.clone();
        self.lru.record(key.clone(), next_epoch);
        self.compact_lru_if_needed();
        Some(value)
    }

    pub(super) fn lookup_ref(&mut self, key: &K) -> Option<&V> {
        let next_epoch = self.next_epoch();
        if let Some(entry) = self.entries.get_mut(key) {
            entry.last_access = next_epoch;
        } else {
            return None;
        }
        self.lru.record(key.clone(), next_epoch);
        self.compact_lru_if_needed();
        self.entries.get(key).map(|entry| &entry.value)
    }

    pub(super) fn insert(&mut self, key: K, value: V, entry_bytes: usize) {
        if self.max_entries == 0 || entry_bytes > self.max_bytes {
            self.remove(&key);
            return;
        }
        self.remove(&key);
        while self.entries.len() >= self.max_entries
            || self.bytes.checked_add(entry_bytes).unwrap_or(usize::MAX) > self.max_bytes
        {
            let Some(evict_key) = self.pop_lru_key() else {
                break;
            };
            self.remove(&evict_key);
        }
        let last_access = self.next_epoch();
        self.bytes = self.bytes.saturating_add(entry_bytes);
        self.entries.insert(
            key.clone(),
            ByteLruEntry {
                value,
                bytes: entry_bytes,
                last_access,
            },
        );
        self.lru.record(key, last_access);
        self.compact_lru_if_needed();
    }

    #[cfg(test)]
    pub(super) fn len(&self) -> usize {
        self.entries.len()
    }

    #[cfg(test)]
    pub(super) fn byte_len(&self) -> usize {
        self.bytes
    }

    #[cfg(test)]
    pub(super) fn contains_key(&self, key: &K) -> bool {
        self.entries.contains_key(key)
    }

    #[cfg(test)]
    pub(super) fn lru_index_len(&self) -> usize {
        self.lru.len()
    }

    fn remove(&mut self, key: &K) -> Option<V> {
        let entry = self.entries.remove(key)?;
        self.bytes = self.bytes.saturating_sub(entry.bytes);
        Some(entry.value)
    }

    fn next_epoch(&mut self) -> u64 {
        self.epoch = self.epoch.saturating_add(1);
        self.epoch
    }

    fn pop_lru_key(&mut self) -> Option<K> {
        self.lru.pop_valid(|key, last_access| {
            self.entries
                .get(key)
                .is_some_and(|entry| entry.last_access == last_access)
        })
    }

    fn compact_lru_if_needed(&mut self) {
        let live = self.entries.len();
        self.lru.compact_if_needed(
            live,
            self.entries
                .iter()
                .map(|(key, entry)| (key.clone(), entry.last_access)),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::{ByteBoundLruCache, ByteLruPanicLabels};

    const LABELS: ByteLruPanicLabels = ByteLruPanicLabels {
        byte_add_overflow: "test byte add overflow",
        byte_sub_underflow: "test byte sub underflow",
        epoch_overflow: "test epoch overflow",
    };

    #[test]
    fn byte_bound_lru_evicts_lru_to_entry_and_byte_budgets() {
        let mut cache = ByteBoundLruCache::new(3, 11, LABELS);
        cache.insert(1u32, "a", 3);
        cache.insert(2u32, "b", 3);
        cache.insert(3u32, "c", 3);
        assert_eq!(cache.lookup_cloned(&1), Some("a"));
        cache.insert(4u32, "d", 5);

        assert!(cache.contains_key(&1));
        assert!(!cache.contains_key(&2));
        assert!(cache.contains_key(&3));
        assert!(cache.contains_key(&4));
        assert!(cache.byte_len() <= 11);
    }

    #[test]
    fn generated_byte_bound_lru_compacts_stale_touches_to_live_scale() {
        let mut cache = ByteBoundLruCache::new(8, usize::MAX, LABELS);

        for id in 0..4096u32 {
            cache.insert(id, id, 1);
            assert_eq!(cache.lookup_cloned(&id), Some(id));
        }

        assert_eq!(cache.len(), 8);
        assert!(
            cache.lru_index_len() <= cache.len().saturating_mul(4).max(8),
            "Fix: generic GPU preprocessor LRU core must compact stale touches to cache-capacity scale"
        );
    }

    #[test]
    fn byte_bound_lru_rejects_single_entry_over_byte_budget() {
        let mut cache = ByteBoundLruCache::new(8, 4, LABELS);
        cache.insert(7u32, "oversized", 5);
        assert_eq!(cache.len(), 0);
        assert_eq!(cache.byte_len(), 0);
    }
}
