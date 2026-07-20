//! Structural contracts for the C macro expansion cache.

mod support;

use support::{assert_byte_lru_core_rejects_and_accounts, crate_file};

#[test]
fn macro_expansion_caches_are_byte_bounded() {
    let model = crate_file("src/parsing/c/preprocess/gpu_pipeline/macro_expansion/model.rs");
    assert!(
        model.contains("const MACRO_EXPANDED_SEGMENT_CACHE_MAX_BYTES: usize"),
        "Fix: expanded macro segment cache must have an explicit byte budget."
    );
    assert!(
        model.contains("const PACKED_MACRO_TABLE_CACHE_MAX_BYTES: usize"),
        "Fix: packed macro table cache must have an explicit byte budget."
    );
    assert!(
        model.contains("cached_expanded_segment_bytes(&value)")
            && model.contains("classified_tokens_bytes(&segment.classified)"),
        "Fix: expanded segment cache must size expanded bytes plus classified token residency."
    );
    assert!(
        model.contains("ByteBoundLruCache<MacroSegmentCacheKey, CachedExpandedSegment>")
            && model.contains("ByteBoundLruCache<[u8; 16], macro_table::PackedMacroTable>"),
        "Fix: macro expansion caches must use the shared byte-bounded LRU core instead of bespoke entry-only caching."
    );
    assert_byte_lru_core_rejects_and_accounts();
    assert!(
        model.contains("value.byte_len()"),
        "Fix: packed macro table cache must size packed table buffers before storing."
    );
    assert!(
        model.contains("packed macro table cache entry is {entry_bytes} bytes"),
        "Fix: oversized packed macro table entries must fail with an actionable byte-budget error."
    );

    let macro_table = crate_file("src/parsing/c/preprocess/gpu_pipeline/macro_table.rs");
    assert!(
        macro_table.contains("pub(crate) fn byte_len(&self) -> usize"),
        "Fix: packed macro tables must expose byte sizing to cache admission."
    );
}
