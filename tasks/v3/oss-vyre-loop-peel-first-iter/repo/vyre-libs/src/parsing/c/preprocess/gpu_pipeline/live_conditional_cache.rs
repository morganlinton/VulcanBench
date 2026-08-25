use super::byte_lru_cache::{ByteBoundLruCache, ByteLruPanicLabels};

const LIVE_CONDITIONAL_CACHE_MAX_ENTRIES: usize = 16_384;
const LIVE_CONDITIONAL_CACHE_MAX_BYTES: usize = 4 * 1024 * 1024;

const LIVE_CONDITIONAL_CACHE_LABELS: ByteLruPanicLabels = ByteLruPanicLabels {
    byte_add_overflow: "vyre-libs gpu preprocessor live conditional cache byte accounting overflowed during insert. Fix: lower live conditional cache limits or shard preprocessing sessions.",
    byte_sub_underflow: "vyre-libs gpu preprocessor live conditional cache byte accounting underflowed during eviction. Fix: repair live conditional cache accounting before relying on memory limits.",
    epoch_overflow: "vyre-libs gpu preprocessor live conditional cache epoch overflowed. Fix: recreate the process-local preprocessor cache before continuing an unbounded translation-unit stream.",
};

#[derive(Clone, Hash, PartialEq, Eq)]
pub(super) struct LiveConditionalCacheKey {
    pub(super) evaluator: u8,
    pub(super) directive_kind: u32,
    pub(super) negated: bool,
    pub(super) row_fingerprint: [u8; 16],
    pub(super) row_len: u32,
    pub(super) macro_fingerprint: [u8; 16],
    pub(super) macro_names_len: u32,
    pub(super) num_macros: u32,
}

pub(super) struct LiveConditionalCache {
    inner: ByteBoundLruCache<LiveConditionalCacheKey, bool>,
}

impl LiveConditionalCache {
    pub(super) fn new() -> Self {
        Self {
            inner: ByteBoundLruCache::new(
                LIVE_CONDITIONAL_CACHE_MAX_ENTRIES,
                LIVE_CONDITIONAL_CACHE_MAX_BYTES,
                LIVE_CONDITIONAL_CACHE_LABELS,
            ),
        }
    }

    #[cfg(test)]
    pub(super) fn with_limit(max_entries: usize) -> Self {
        Self {
            inner: ByteBoundLruCache::new(
                max_entries,
                LIVE_CONDITIONAL_CACHE_MAX_BYTES,
                LIVE_CONDITIONAL_CACHE_LABELS,
            ),
        }
    }

    #[cfg(test)]
    pub(super) fn with_limits(max_entries: usize, max_bytes: usize) -> Self {
        Self {
            inner: ByteBoundLruCache::new(max_entries, max_bytes, LIVE_CONDITIONAL_CACHE_LABELS),
        }
    }

    pub(super) fn lookup(&mut self, key: &LiveConditionalCacheKey) -> Option<bool> {
        self.inner.lookup_cloned(key)
    }

    pub(super) fn insert(&mut self, key: LiveConditionalCacheKey, value: bool) {
        let entry_bytes = live_conditional_entry_bytes();
        self.inner.insert(key, value, entry_bytes);
    }

    #[cfg(test)]
    pub(super) fn len(&self) -> usize {
        self.inner.len()
    }

    #[cfg(test)]
    pub(super) fn byte_len(&self) -> usize {
        self.inner.byte_len()
    }

    #[cfg(test)]
    pub(super) fn contains_key(&self, key: &LiveConditionalCacheKey) -> bool {
        self.inner.contains_key(key)
    }

    #[cfg(test)]
    pub(super) fn lru_index_len(&self) -> usize {
        self.inner.lru_index_len()
    }
}

fn live_conditional_entry_bytes() -> usize {
    std::mem::size_of::<LiveConditionalCacheKey>()
        .checked_add(std::mem::size_of::<bool>())
        .and_then(|bytes| bytes.checked_add(std::mem::size_of::<u64>()))
        .unwrap_or(usize::MAX)
}

#[cfg(test)]
mod tests {
    use super::{live_conditional_entry_bytes, LiveConditionalCache, LiveConditionalCacheKey};

    fn key(id: u8) -> LiveConditionalCacheKey {
        LiveConditionalCacheKey {
            evaluator: id,
            directive_kind: id as u32,
            negated: false,
            row_fingerprint: [id; 16],
            row_len: id as u32,
            macro_fingerprint: [id; 16],
            macro_names_len: id as u32,
            num_macros: id as u32,
        }
    }

    #[test]
    fn live_conditional_cache_evicts_least_recently_used_entry() {
        let mut cache = LiveConditionalCache::with_limit(2);
        let a = key(1);
        let b = key(2);
        let c = key(3);
        cache.insert(a.clone(), true);
        cache.insert(b.clone(), false);
        assert_eq!(cache.lookup(&a), Some(true));
        cache.insert(c.clone(), true);
        assert!(cache.contains_key(&a));
        assert!(!cache.contains_key(&b));
        assert!(cache.contains_key(&c));
        assert_eq!(cache.len(), 2);
    }

    #[test]
    fn live_conditional_cache_evicts_to_byte_budget() {
        let entry_bytes = live_conditional_entry_bytes();
        let mut cache = LiveConditionalCache::with_limits(8, entry_bytes * 2);
        let a = key(1);
        let b = key(2);
        let c = key(3);
        cache.insert(a.clone(), true);
        cache.insert(b.clone(), false);
        assert_eq!(cache.lookup(&a), Some(true));
        cache.insert(c.clone(), true);
        assert!(cache.contains_key(&a));
        assert!(!cache.contains_key(&b));
        assert!(cache.contains_key(&c));
        assert_eq!(cache.len(), 2);
        assert!(cache.byte_len() <= entry_bytes * 2);
    }

    #[test]
    fn live_conditional_cache_lru_index_stays_capacity_scale() {
        let mut cache = LiveConditionalCache::with_limit(4);

        for id in 0..96u8 {
            let cache_key = key(id);
            cache.insert(cache_key.clone(), id % 2 == 0);
            assert!(cache.lookup(&cache_key).is_some());
        }

        assert_eq!(cache.len(), 4);
        assert!(
            cache.lru_index_len() <= cache.len().saturating_mul(4).max(8),
            "Fix: live conditional cache LRU index must compact stale touches to cache-capacity scale"
        );
    }
}
