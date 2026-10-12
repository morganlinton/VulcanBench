use super::*;

pub(super) struct BoundedPipelineCache<K, V>
where
    K: Clone + Eq + Hash,
{
    entries: HashMap<K, CacheEntry<V>>,
    bytes: usize,
    clock: u64,
}

struct CacheEntry<V> {
    value: V,
    bytes: usize,
    last_seen: u64,
}

impl<K, V> Default for BoundedPipelineCache<K, V>
where
    K: Clone + Eq + Hash,
{
    fn default() -> Self {
        Self {
            entries: HashMap::new(),
            bytes: 0,
            clock: 0,
        }
    }
}

impl<K, V> BoundedPipelineCache<K, V>
where
    K: Clone + Eq + Hash,
{
    pub(super) fn get_cloned(&mut self, key: &K) -> Option<V>
    where
        V: Clone,
    {
        let tick = self.next_tick();
        let entry = self.entries.get_mut(key)?;
        entry.last_seen = tick;
        Some(entry.value.clone())
    }

    pub(super) fn insert(&mut self, key: K, value: V, max_entries: usize) {
        self.insert_with_cost(key, value, max_entries, 0, usize::MAX);
    }

    pub(super) fn insert_with_cost(
        &mut self,
        key: K,
        value: V,
        max_entries: usize,
        entry_bytes: usize,
        max_bytes: usize,
    ) {
        if max_entries == 0 || entry_bytes > max_bytes {
            self.remove(&key);
            return;
        }
        self.remove(&key);
        let tick = self.next_tick();
        self.bytes = self.bytes.saturating_add(entry_bytes);
        self.entries.insert(
            key.clone(),
            CacheEntry {
                value,
                bytes: entry_bytes,
                last_seen: tick,
            },
        );
        while self.entries.len() > max_entries || self.bytes > max_bytes {
            let Some(evict_key) = self
                .entries
                .iter()
                .filter(|(candidate, _)| *candidate != &key)
                .min_by_key(|(_, entry)| entry.last_seen)
                .map(|(candidate, _)| candidate.clone())
            else {
                break;
            };
            self.remove(&evict_key);
        }
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.entries.len()
    }

    #[cfg(test)]
    fn get(&self, key: &K) -> Option<&V> {
        self.entries.get(key).map(|entry| &entry.value)
    }

    #[cfg(test)]
    fn byte_len(&self) -> usize {
        self.bytes
    }

    fn remove(&mut self, key: &K) -> Option<CacheEntry<V>> {
        let entry = self.entries.remove(key)?;
        self.bytes = self.bytes.saturating_sub(entry.bytes);
        Some(entry)
    }

    fn next_tick(&mut self) -> u64 {
        if self.clock == u64::MAX {
            self.clock = 0;
            for entry in self.entries.values_mut() {
                entry.last_seen = 0;
            }
        }
        self.clock += 1;
        self.clock
    }
}

#[cfg(test)]
mod tests {
    use super::BoundedPipelineCache;

    #[test]
    fn bounded_cache_rejects_zero_entry_budget() {
        let mut cache = BoundedPipelineCache::default();
        cache.insert(1u32, 10u32, 0);
        assert_eq!(cache.len(), 0);
    }

    #[test]
    fn bounded_cache_evicts_to_entry_budget() {
        let mut cache = BoundedPipelineCache::default();
        cache.insert(1u32, 10u32, 2);
        cache.insert(2u32, 20u32, 2);
        cache.insert(3u32, 30u32, 2);
        assert!(cache.len() <= 2);
        assert_eq!(cache.get(&3), Some(&30));
    }

    #[test]
    fn bounded_cache_replacement_does_not_count_twice() {
        let mut cache = BoundedPipelineCache::default();
        cache.insert(1u32, 10u32, 1);
        cache.insert(1u32, 11u32, 1);
        assert_eq!(cache.len(), 1);
        assert_eq!(cache.get(&1), Some(&11));
    }

    #[test]
    fn bounded_cache_hit_refreshes_lru_position() {
        let mut cache = BoundedPipelineCache::default();
        cache.insert(1u32, 10u32, 2);
        cache.insert(2u32, 20u32, 2);
        assert_eq!(cache.get_cloned(&1), Some(10));
        cache.insert(3u32, 30u32, 2);
        assert_eq!(cache.get(&1), Some(&10));
        assert_eq!(cache.get(&2), None);
        assert_eq!(cache.get(&3), Some(&30));
    }

    #[test]
    fn bounded_cache_rejects_oversized_cost() {
        let mut cache = BoundedPipelineCache::default();
        cache.insert_with_cost(1u32, 10u32, 8, 17, 16);
        assert_eq!(cache.len(), 0);
        assert_eq!(cache.byte_len(), 0);
    }

    #[test]
    fn bounded_cache_evicts_to_byte_budget() {
        let mut cache = BoundedPipelineCache::default();
        cache.insert_with_cost(1u32, 10u32, 8, 8, 16);
        cache.insert_with_cost(2u32, 20u32, 8, 8, 16);
        assert_eq!(cache.get_cloned(&1), Some(10));
        cache.insert_with_cost(3u32, 30u32, 8, 8, 16);
        assert_eq!(cache.get(&1), Some(&10));
        assert_eq!(cache.get(&2), None);
        assert_eq!(cache.get(&3), Some(&30));
        assert_eq!(cache.byte_len(), 16);
    }
}
