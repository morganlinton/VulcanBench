//! Structural contracts for the live conditional preprocessing cache.

use std::fs;
use std::path::Path;

fn crate_file(path: &str) -> String {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    fs::read_to_string(manifest.join(path)).unwrap_or_else(|error| {
        panic!("failed to read {path}: {error}");
    })
}

fn live_conditional_cache_source() -> String {
    crate_file("src/parsing/c/preprocess/gpu_pipeline/live_conditional_cache.rs")
}

#[test]
fn live_conditional_cache_is_entry_and_byte_bounded() {
    let source = live_conditional_cache_source();
    assert!(
        source.contains("const LIVE_CONDITIONAL_CACHE_MAX_BYTES: usize"),
        "Fix: live conditional cache must have an explicit byte budget, not only an entry count."
    );
    assert!(
        source.contains("ByteBoundLruCache<LiveConditionalCacheKey, bool>"),
        "Fix: live conditional cache must use the shared byte-bounded LRU core instead of bespoke entry-only caching."
    );
    assert!(
        source.contains("fn live_conditional_entry_bytes() -> usize"),
        "Fix: fixed-size live conditional cache entries must still have explicit byte accounting."
    );
    let byte_lru_cache = crate_file("src/parsing/c/preprocess/gpu_pipeline/byte_lru_cache.rs");
    assert!(
        byte_lru_cache.contains("bytes: usize") && byte_lru_cache.contains("max_bytes: usize"),
        "Fix: shared GPU preprocessor cache core must track resident bytes and byte limit."
    );
    assert!(
        byte_lru_cache.contains("entry_bytes > self.max_bytes")
            && byte_lru_cache.contains(".checked_add(entry_bytes)")
            && byte_lru_cache.contains("saturating_sub(entry.bytes)"),
        "Fix: shared GPU preprocessor cache core must reject oversized entries, evict to byte budget, and update byte accounting."
    );
}
