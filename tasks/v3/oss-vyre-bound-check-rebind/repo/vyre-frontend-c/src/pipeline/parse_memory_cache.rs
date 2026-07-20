use std::collections::HashMap;

use crate::api::CParseSummary;

use super::parse_cache::{CacheKey, CachedLexerOutputs};

const LEXER_OUTPUT_CACHE_MAX_ENTRIES: usize = 256;
const LEXER_OUTPUT_CACHE_MAX_BYTES: usize = 512 * 1024 * 1024;
const SUMMARY_CACHE_MAX_ENTRIES: usize = 4096;

pub(crate) struct LexerOutputCache {
    entries: HashMap<CacheKey, LexerOutputCacheEntry>,
    bytes: usize,
    max_entries: usize,
    max_bytes: usize,
    epoch: u64,
}

struct LexerOutputCacheEntry {
    value: CachedLexerOutputs,
    bytes: usize,
    last_access: u64,
}

impl LexerOutputCache {
    pub(crate) fn new() -> Self {
        Self {
            entries: HashMap::new(),
            bytes: 0,
            max_entries: LEXER_OUTPUT_CACHE_MAX_ENTRIES,
            max_bytes: LEXER_OUTPUT_CACHE_MAX_BYTES,
            epoch: 0,
        }
    }

    #[cfg(test)]
    pub(crate) fn with_limits(max_entries: usize, max_bytes: usize) -> Self {
        Self {
            entries: HashMap::new(),
            bytes: 0,
            max_entries,
            max_bytes,
            epoch: 0,
        }
    }

    pub(crate) fn lookup(&mut self, key: &CacheKey) -> Option<CachedLexerOutputs> {
        let next_epoch = self.next_epoch();
        let entry = self.entries.get_mut(key)?;
        entry.last_access = next_epoch;
        Some(entry.value.clone())
    }

    pub(crate) fn insert(&mut self, key: CacheKey, value: CachedLexerOutputs) {
        let entry_bytes = lexer_output_entry_bytes(&value);
        if self.max_entries == 0 || entry_bytes > self.max_bytes {
            self.remove(&key);
            return;
        }
        self.remove(&key);
        while self.entries.len() >= self.max_entries
            || self.bytes.checked_add(entry_bytes).unwrap_or(usize::MAX) > self.max_bytes
        {
            let Some(evict_key) = self.least_recently_used_key() else {
                break;
            };
            self.remove(&evict_key);
        }
        let last_access = self.next_epoch();
        self.bytes = self.bytes.saturating_add(entry_bytes);
        self.entries.insert(
            key,
            LexerOutputCacheEntry {
                value,
                bytes: entry_bytes,
                last_access,
            },
        );
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.entries.len()
    }

    #[cfg(test)]
    fn byte_len(&self) -> usize {
        self.bytes
    }

    #[cfg(test)]
    fn contains_key(&self, key: &CacheKey) -> bool {
        self.entries.contains_key(key)
    }

    fn remove(&mut self, key: &CacheKey) -> Option<LexerOutputCacheEntry> {
        let entry = self.entries.remove(key)?;
        self.bytes = self.bytes.saturating_sub(entry.bytes);
        Some(entry)
    }

    fn next_epoch(&mut self) -> u64 {
        self.epoch = self.epoch.saturating_add(1);
        self.epoch
    }

    fn least_recently_used_key(&self) -> Option<CacheKey> {
        self.entries
            .iter()
            .min_by_key(|(_, entry)| entry.last_access)
            .map(|(key, _)| *key)
    }
}

fn lexer_output_entry_bytes(entry: &CachedLexerOutputs) -> usize {
    let keyword_bytes = entry
        .cuda_keyword_haystack
        .as_ref()
        .map_or(0usize, |(bytes, _)| bytes.len());
    entry
        .types
        .len()
        .saturating_add(entry.starts.len())
        .saturating_add(entry.lens.len())
        .saturating_add(entry.counts.len())
        .saturating_add(keyword_bytes)
}

pub(crate) struct SummaryCache {
    entries: HashMap<CacheKey, SummaryCacheEntry>,
    max_entries: usize,
    epoch: u64,
}

struct SummaryCacheEntry {
    value: CParseSummary,
    last_access: u64,
}

impl SummaryCache {
    pub(crate) fn new() -> Self {
        Self {
            entries: HashMap::new(),
            max_entries: SUMMARY_CACHE_MAX_ENTRIES,
            epoch: 0,
        }
    }

    #[cfg(test)]
    pub(crate) fn with_limit(max_entries: usize) -> Self {
        Self {
            entries: HashMap::new(),
            max_entries,
            epoch: 0,
        }
    }

    pub(crate) fn lookup(&mut self, key: &CacheKey) -> Option<CParseSummary> {
        let next_epoch = self.next_epoch();
        let entry = self.entries.get_mut(key)?;
        entry.last_access = next_epoch;
        Some(entry.value)
    }

    pub(crate) fn insert(&mut self, key: CacheKey, value: CParseSummary) {
        if self.max_entries == 0 {
            self.entries.remove(&key);
            return;
        }
        self.entries.remove(&key);
        while self.entries.len() >= self.max_entries {
            let Some(evict_key) = self.least_recently_used_key() else {
                break;
            };
            self.entries.remove(&evict_key);
        }
        let last_access = self.next_epoch();
        self.entries
            .insert(key, SummaryCacheEntry { value, last_access });
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.entries.len()
    }

    #[cfg(test)]
    fn contains_key(&self, key: &CacheKey) -> bool {
        self.entries.contains_key(key)
    }

    fn next_epoch(&mut self) -> u64 {
        self.epoch = self.epoch.saturating_add(1);
        self.epoch
    }

    fn least_recently_used_key(&self) -> Option<CacheKey> {
        self.entries
            .iter()
            .min_by_key(|(_, entry)| entry.last_access)
            .map(|(key, _)| *key)
    }
}

#[cfg(test)]
mod tests {
    use super::{LexerOutputCache, SummaryCache};
    use crate::api::CParseSummary;
    use crate::pipeline::parse_cache::CachedLexerOutputs;

    fn fixture_summary() -> CParseSummary {
        CParseSummary {
            source_bytes: 1234,
            token_count: 5,
            ast_bytes: 678,
            ast_node_count: 9,
            vast_bytes: 10,
            abi_layout_bytes: 11,
            expression_shape_bytes: 12,
            program_graph_bytes: 13,
            semantic_node_bytes: 14,
            semantic_edge_bytes: 15,
            sema_scope_bytes: 16,
            function_record_bytes: 17,
            call_record_bytes: 18,
        }
    }

    fn cached_lexer(bytes: usize) -> CachedLexerOutputs {
        CachedLexerOutputs {
            types: std::sync::Arc::<[u8]>::from(vec![1; bytes]),
            starts: std::sync::Arc::<[u8]>::from(Vec::<u8>::new()),
            lens: std::sync::Arc::<[u8]>::from(Vec::<u8>::new()),
            counts: std::sync::Arc::<[u8]>::from(Vec::<u8>::new()),
            n_tokens: bytes as u32,
            keyword_promoted: true,
            cuda_keyword_haystack: None,
        }
    }

    fn cached_cuda_lexer(bytes: usize, packed_haystack_bytes: usize) -> CachedLexerOutputs {
        CachedLexerOutputs {
            types: std::sync::Arc::<[u8]>::from(vec![1; bytes]),
            starts: std::sync::Arc::<[u8]>::from(Vec::<u8>::new()),
            lens: std::sync::Arc::<[u8]>::from(Vec::<u8>::new()),
            counts: std::sync::Arc::<[u8]>::from(Vec::<u8>::new()),
            n_tokens: bytes as u32,
            keyword_promoted: true,
            cuda_keyword_haystack: Some((
                std::sync::Arc::<[u8]>::from(vec![7; packed_haystack_bytes]),
                packed_haystack_bytes as u32 / 4,
            )),
        }
    }

    #[test]
    fn lexer_output_cache_hit_reuses_shared_columns_without_deep_copy() {
        let mut cache = LexerOutputCache::with_limits(4, 64);
        let key = [9; 16];
        cache.insert(key, cached_lexer(16));
        let first = cache.lookup(&key).expect("Fix: first cache hit");
        let second = cache.lookup(&key).expect("Fix: second cache hit");
        assert!(
            std::sync::Arc::ptr_eq(&first.types, &second.types),
            "cached token types must be shared across hits instead of deep-copied"
        );
        assert!(
            std::sync::Arc::ptr_eq(&first.starts, &second.starts),
            "cached token starts must be shared across hits instead of deep-copied"
        );
        assert!(
            std::sync::Arc::ptr_eq(&first.lens, &second.lens),
            "cached token lengths must be shared across hits instead of deep-copied"
        );
        assert!(
            std::sync::Arc::ptr_eq(&first.counts, &second.counts),
            "cached token counts must be shared across hits instead of deep-copied"
        );
    }

    #[test]
    fn lexer_output_cache_hit_reuses_cuda_packed_haystack_without_deep_copy() {
        let mut cache = LexerOutputCache::with_limits(4, 64);
        let key = [8; 16];
        cache.insert(key, cached_cuda_lexer(16, 32));
        let first = cache.lookup(&key).expect("Fix: first CUDA cache hit");
        let second = cache.lookup(&key).expect("Fix: second CUDA cache hit");
        let (first_haystack, first_len) = first
            .cuda_keyword_haystack
            .as_ref()
            .expect("Fix: CUDA cache fixture must retain packed haystack");
        let (second_haystack, second_len) = second
            .cuda_keyword_haystack
            .as_ref()
            .expect("Fix: CUDA cache fixture must retain packed haystack");
        assert_eq!(*first_len, *second_len);
        assert!(
            std::sync::Arc::ptr_eq(first_haystack, second_haystack),
            "cached CUDA packed haystack must be shared across hits instead of deep-copied or host-repacked"
        );
    }

    #[test]
    fn lexer_output_cache_rejects_oversized_entry() {
        let mut cache = LexerOutputCache::with_limits(4, 8);
        cache.insert([1; 16], cached_lexer(9));
        assert_eq!(cache.len(), 0);
        assert_eq!(cache.byte_len(), 0);
    }

    #[test]
    fn lexer_output_cache_evicts_least_recently_used_entry_to_byte_budget() {
        let mut cache = LexerOutputCache::with_limits(4, 8);
        cache.insert([1; 16], cached_lexer(4));
        cache.insert([2; 16], cached_lexer(4));
        assert!(cache.lookup(&[1; 16]).is_some());
        cache.insert([3; 16], cached_lexer(4));
        assert!(cache.contains_key(&[1; 16]));
        assert!(!cache.contains_key(&[2; 16]));
        assert!(cache.contains_key(&[3; 16]));
        assert_eq!(cache.byte_len(), 8);
    }

    #[test]
    fn lexer_output_cache_replacement_does_not_double_count() {
        let mut cache = LexerOutputCache::with_limits(4, 8);
        cache.insert([1; 16], cached_lexer(6));
        cache.insert([1; 16], cached_lexer(6));
        assert_eq!(cache.len(), 1);
        assert_eq!(cache.byte_len(), 6);
    }

    #[test]
    fn summary_cache_evicts_least_recently_used_entry() {
        let mut cache = SummaryCache::with_limit(2);
        cache.insert([1; 16], fixture_summary());
        cache.insert([2; 16], fixture_summary());
        assert!(cache.lookup(&[1; 16]).is_some());
        cache.insert([3; 16], fixture_summary());
        assert!(cache.contains_key(&[1; 16]));
        assert!(!cache.contains_key(&[2; 16]));
        assert!(cache.contains_key(&[3; 16]));
        assert_eq!(cache.len(), 2);
    }
}
