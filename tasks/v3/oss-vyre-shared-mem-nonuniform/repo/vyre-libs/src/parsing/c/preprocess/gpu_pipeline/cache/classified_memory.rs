use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};

use super::super::byte_lru_cache::{ByteBoundLruCache, ByteLruPanicLabels};
use super::super::classified_size::classified_tokens_bytes;
use super::super::ClassifiedTokens;
#[cfg(test)]
use super::disk_common::source_hash128;

const CLASSIFIED_CACHE_MAX_ENTRIES: usize = 4096;
const CLASSIFIED_CACHE_MAX_BYTES: usize = 512 * 1024 * 1024;

pub(crate) const PREPROCESS_CACHE_SEMANTIC_VERSION: &[u8] = b"gpu-preprocess-cache-v14";

const CLASSIFIED_CACHE_LABELS: ByteLruPanicLabels = ByteLruPanicLabels {
    byte_add_overflow: "vyre-libs gpu preprocessor classified token cache byte accounting overflowed during insert. Fix: lower classified token cache limits or shard preprocessing sessions.",
    byte_sub_underflow: "vyre-libs gpu preprocessor classified token cache byte accounting underflowed during eviction. Fix: repair classified token cache accounting before relying on memory limits.",
    epoch_overflow: "vyre-libs gpu preprocessor classified token cache epoch overflowed. Fix: recreate process-local preprocess cache before continuing an unbounded translation-unit stream.",
};

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub(crate) struct ClassifiedCacheKey {
    pub(crate) path: PathBuf,
    pub(crate) source_len: usize,
    pub(crate) source_hash: [u8; 16],
}

pub(super) struct ClassifiedTokenCache {
    inner: ByteBoundLruCache<ClassifiedCacheKey, Arc<ClassifiedTokens>>,
}

impl ClassifiedTokenCache {
    fn new() -> Self {
        Self {
            inner: ByteBoundLruCache::new(
                CLASSIFIED_CACHE_MAX_ENTRIES,
                CLASSIFIED_CACHE_MAX_BYTES,
                CLASSIFIED_CACHE_LABELS,
            ),
        }
    }

    #[cfg(test)]
    pub(super) fn with_limit(max_entries: usize) -> Self {
        Self {
            inner: ByteBoundLruCache::new(
                max_entries,
                CLASSIFIED_CACHE_MAX_BYTES,
                CLASSIFIED_CACHE_LABELS,
            ),
        }
    }

    #[cfg(test)]
    pub(super) fn with_limits(max_entries: usize, max_bytes: usize) -> Self {
        Self {
            inner: ByteBoundLruCache::new(max_entries, max_bytes, CLASSIFIED_CACHE_LABELS),
        }
    }

    pub(super) fn lookup(&mut self, key: &ClassifiedCacheKey) -> Option<Arc<ClassifiedTokens>> {
        self.inner.lookup_cloned(key)
    }

    pub(super) fn insert(&mut self, key: ClassifiedCacheKey, value: Arc<ClassifiedTokens>) {
        let entry_bytes = classified_tokens_bytes(&value);
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
    pub(super) fn contains_key(&self, key: &ClassifiedCacheKey) -> bool {
        self.inner.contains_key(key)
    }

    #[cfg(test)]
    pub(super) fn lru_index_len(&self) -> usize {
        self.inner.lru_index_len()
    }
}

fn classified_cache() -> &'static Mutex<ClassifiedTokenCache> {
    static CACHE: OnceLock<Mutex<ClassifiedTokenCache>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(ClassifiedTokenCache::new()))
}

#[cfg(test)]
pub(crate) fn classified_cache_key(path: &std::path::Path, source: &[u8]) -> ClassifiedCacheKey {
    classified_cache_key_from_hash(path, source.len(), source_hash128(source))
}

pub(crate) fn classified_cache_key_from_hash(
    path: &std::path::Path,
    source_len: usize,
    source_hash: [u8; 16],
) -> ClassifiedCacheKey {
    ClassifiedCacheKey {
        path: path.to_path_buf(),
        source_len,
        source_hash,
    }
}

pub(crate) fn cached_classified_tokens(
    key: &ClassifiedCacheKey,
) -> Result<Option<Arc<ClassifiedTokens>>, String> {
    classified_cache()
        .lock()
        .map_err(|_| "vyre-libs::gpu_pipeline: classified token cache poisoned".to_string())
        .map(|mut cache| cache.lookup(key))
}

pub(crate) fn insert_classified_tokens(
    key: ClassifiedCacheKey,
    classified: Arc<ClassifiedTokens>,
) -> Result<(), String> {
    let mut cache = classified_cache().lock().map_err(|_| {
        "vyre-libs::gpu_pipeline: classified token cache poisoned while inserting".to_string()
    })?;
    cache.insert(key, classified);
    Ok(())
}
