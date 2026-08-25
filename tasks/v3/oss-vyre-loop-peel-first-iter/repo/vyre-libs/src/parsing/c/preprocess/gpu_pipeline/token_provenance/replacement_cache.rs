use super::*;
use crate::parsing::c::preprocess::gpu_pipeline::byte_lru_cache::{
    ByteBoundLruCache, ByteLruPanicLabels,
};
use crate::parsing::c::preprocess::gpu_pipeline::classified_size::classified_tokens_bytes;
use crate::parsing::c::preprocess::gpu_pipeline::token_provenance::model::REPLACEMENT_TOKEN_CACHE_MAX_BYTES;

const REPLACEMENT_TOKEN_CACHE_LABELS: ByteLruPanicLabels = ByteLruPanicLabels {
    byte_add_overflow: "vyre-libs gpu preprocessor replacement token cache byte accounting overflowed during insert. Fix: lower replacement token cache limits or shard macro-expansion sessions.",
    byte_sub_underflow: "vyre-libs gpu preprocessor replacement token cache byte accounting underflowed during eviction. Fix: repair replacement token cache accounting before relying on memory limits.",
    epoch_overflow: "vyre-libs gpu preprocessor replacement token cache epoch overflowed. Fix: recreate process-local token provenance cache before continuing an unbounded macro-expansion stream.",
};

pub(crate) fn cached_replacement_tokens(
    dispatcher: &dyn GpuDispatcher,
    mac: &MacroDef,
    symbol_id: [u8; 16],
) -> Result<std::sync::Arc<ClassifiedTokens>, String> {
    let key = ReplacementTokenCacheKey {
        symbol_id,
        body_hash: hash_bytes16(&mac.body),
        args_hash: hash_bytes16(&mac.args),
        is_function_like: mac.is_function_like,
    };
    if let Some(classified) = replacement_token_cache()
        .lock()
        .map_err(|error| format!("macro replacement token cache lock poisoned: {error}"))?
        .lookup(&key)
    {
        return Ok(classified);
    }
    let classified = std::sync::Arc::new(gpu_tokenize_without_directive_metadata(
        dispatcher, &mac.body,
    )?);
    let mut cache = replacement_token_cache()
        .lock()
        .map_err(|error| format!("macro replacement token cache lock poisoned: {error}"))?;
    cache.insert(key, classified.clone());
    Ok(classified)
}

struct ReplacementTokenCache {
    inner: ByteBoundLruCache<ReplacementTokenCacheKey, std::sync::Arc<ClassifiedTokens>>,
}

impl ReplacementTokenCache {
    fn new() -> Self {
        Self {
            inner: ByteBoundLruCache::new(
                REPLACEMENT_TOKEN_CACHE_MAX_ENTRIES,
                REPLACEMENT_TOKEN_CACHE_MAX_BYTES,
                REPLACEMENT_TOKEN_CACHE_LABELS,
            ),
        }
    }

    #[cfg(test)]
    fn with_limits(max_entries: usize, max_bytes: usize) -> Self {
        Self {
            inner: ByteBoundLruCache::new(max_entries, max_bytes, REPLACEMENT_TOKEN_CACHE_LABELS),
        }
    }

    fn lookup(
        &mut self,
        key: &ReplacementTokenCacheKey,
    ) -> Option<std::sync::Arc<ClassifiedTokens>> {
        self.inner.lookup_cloned(key)
    }

    fn insert(&mut self, key: ReplacementTokenCacheKey, value: std::sync::Arc<ClassifiedTokens>) {
        let entry_bytes = classified_tokens_bytes(&value);
        self.inner.insert(key, value, entry_bytes);
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.inner.len()
    }

    #[cfg(test)]
    fn byte_len(&self) -> usize {
        self.inner.byte_len()
    }

    #[cfg(test)]
    fn contains_key(&self, key: &ReplacementTokenCacheKey) -> bool {
        self.inner.contains_key(key)
    }

    #[cfg(test)]
    fn lru_index_len(&self) -> usize {
        self.inner.lru_index_len()
    }
}

fn replacement_token_cache() -> &'static Mutex<ReplacementTokenCache> {
    static CACHE: OnceLock<Mutex<ReplacementTokenCache>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(ReplacementTokenCache::new()))
}

pub(crate) fn hash_bytes16(bytes: &[u8]) -> [u8; 16] {
    let digest = blake3::hash(bytes);
    let mut out = [0u8; 16];
    out.copy_from_slice(&digest.as_bytes()[..16]);
    out
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;

    fn key(id: u8) -> ReplacementTokenCacheKey {
        ReplacementTokenCacheKey {
            symbol_id: [id; 16],
            body_hash: [id.wrapping_add(1); 16],
            args_hash: [id.wrapping_add(2); 16],
            is_function_like: id % 2 == 0,
        }
    }

    fn classified(id: u8, source_len: usize) -> Arc<ClassifiedTokens> {
        Arc::new(ClassifiedTokens {
            tok_types: vec![id as u32],
            tok_starts: vec![0],
            tok_lens: vec![source_len as u32],
            directive_kinds: vec![0],
            directive_count: 0,
            source: Arc::from(vec![id; source_len].into_boxed_slice()),
        })
    }

    #[test]
    fn replacement_token_cache_evicts_to_byte_budget() {
        let mut cache = ReplacementTokenCache::with_limits(8, 96);
        let a = key(1);
        let b = key(2);
        let c = key(3);
        cache.insert(a.clone(), classified(1, 16));
        cache.insert(b.clone(), classified(2, 16));
        assert!(cache.lookup(&a).is_some());
        cache.insert(c.clone(), classified(3, 48));
        assert!(cache.contains_key(&a));
        assert!(!cache.contains_key(&b));
        assert!(cache.contains_key(&c));
        assert!(cache.byte_len() <= 96);
        assert_eq!(cache.len(), 2);
    }

    #[test]
    fn replacement_token_cache_lru_index_stays_capacity_scale() {
        let mut cache = ReplacementTokenCache::with_limits(4, 1 << 20);

        for id in 0..96u8 {
            let key = key(id);
            cache.insert(key.clone(), classified(id, 8));
            assert!(cache.lookup(&key).is_some());
        }

        assert_eq!(cache.len(), 4);
        assert!(
            cache.lru_index_len() <= cache.len().saturating_mul(4).max(8),
            "Fix: replacement token cache LRU index must compact stale touches to cache-capacity scale"
        );
    }
}
