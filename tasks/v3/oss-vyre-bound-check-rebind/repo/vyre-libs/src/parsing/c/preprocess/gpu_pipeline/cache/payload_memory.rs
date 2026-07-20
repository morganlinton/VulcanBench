use std::sync::{Arc, Mutex, OnceLock};

use super::super::byte_lru_cache::{ByteBoundLruCache, ByteLruPanicLabels};
use super::super::payload_size::directive_payloads_bytes;
use super::super::DirectivePayload;
use super::payload_keys::PayloadsCacheKey;

const PAYLOAD_CACHE_MAX_ENTRIES: usize = 4096;
const PAYLOAD_CACHE_MAX_BYTES: usize = 512 * 1024 * 1024;

const PAYLOAD_CACHE_LABELS: ByteLruPanicLabels = ByteLruPanicLabels {
    byte_add_overflow: "vyre-libs gpu preprocessor directive payload cache byte accounting overflowed during insert. Fix: lower payload cache limits or shard preprocessing sessions.",
    byte_sub_underflow: "vyre-libs gpu preprocessor directive payload cache byte accounting underflowed during eviction. Fix: repair payload cache accounting before relying on memory limits.",
    epoch_overflow: "vyre-libs gpu preprocessor directive payload cache epoch overflowed. Fix: recreate process-local preprocess cache before continuing an unbounded translation-unit stream.",
};

pub(in crate::parsing::c::preprocess::gpu_pipeline) struct PayloadCache {
    inner: ByteBoundLruCache<PayloadsCacheKey, Arc<[DirectivePayload]>>,
}

impl PayloadCache {
    fn new() -> Self {
        Self {
            inner: ByteBoundLruCache::new(
                PAYLOAD_CACHE_MAX_ENTRIES,
                PAYLOAD_CACHE_MAX_BYTES,
                PAYLOAD_CACHE_LABELS,
            ),
        }
    }

    #[cfg(test)]
    pub(in crate::parsing::c::preprocess::gpu_pipeline) fn with_limit(max_entries: usize) -> Self {
        Self {
            inner: ByteBoundLruCache::new(
                max_entries,
                PAYLOAD_CACHE_MAX_BYTES,
                PAYLOAD_CACHE_LABELS,
            ),
        }
    }

    #[cfg(test)]
    pub(in crate::parsing::c::preprocess::gpu_pipeline) fn with_limits(
        max_entries: usize,
        max_bytes: usize,
    ) -> Self {
        Self {
            inner: ByteBoundLruCache::new(max_entries, max_bytes, PAYLOAD_CACHE_LABELS),
        }
    }

    pub(in crate::parsing::c::preprocess::gpu_pipeline) fn lookup(
        &mut self,
        key: &PayloadsCacheKey,
    ) -> Option<Arc<[DirectivePayload]>> {
        self.inner.lookup_cloned(key)
    }

    pub(in crate::parsing::c::preprocess::gpu_pipeline) fn insert(
        &mut self,
        key: PayloadsCacheKey,
        value: Arc<[DirectivePayload]>,
    ) {
        let entry_bytes = directive_payloads_bytes(&value);
        self.inner.insert(key, value, entry_bytes);
    }

    #[cfg(test)]
    pub(in crate::parsing::c::preprocess::gpu_pipeline) fn len(&self) -> usize {
        self.inner.len()
    }

    #[cfg(test)]
    pub(in crate::parsing::c::preprocess::gpu_pipeline) fn byte_len(&self) -> usize {
        self.inner.byte_len()
    }

    #[cfg(test)]
    pub(in crate::parsing::c::preprocess::gpu_pipeline) fn contains_key(
        &self,
        key: &PayloadsCacheKey,
    ) -> bool {
        self.inner.contains_key(key)
    }

    #[cfg(test)]
    pub(in crate::parsing::c::preprocess::gpu_pipeline) fn lru_index_len(&self) -> usize {
        self.inner.lru_index_len()
    }
}

fn payload_cache() -> &'static Mutex<PayloadCache> {
    static CACHE: OnceLock<Mutex<PayloadCache>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(PayloadCache::new()))
}

pub(crate) fn cached_payloads(
    key: &PayloadsCacheKey,
) -> Result<Option<Arc<[DirectivePayload]>>, String> {
    payload_cache()
        .lock()
        .map_err(|_| "vyre-libs::gpu_pipeline: directive payload cache poisoned".to_string())
        .map(|mut cache| cache.lookup(key))
}

pub(crate) fn insert_payloads(
    key: PayloadsCacheKey,
    payloads: Arc<[DirectivePayload]>,
) -> Result<(), String> {
    let mut cache = payload_cache().lock().map_err(|_| {
        "vyre-libs::gpu_pipeline: directive payload cache poisoned while inserting".to_string()
    })?;
    cache.insert(key, payloads);
    Ok(())
}
