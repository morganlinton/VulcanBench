//! Structural contracts for classified-token preprocessing memory caching.

mod support;

use support::{
    assert_byte_lru_core_rejects_and_accounts, assert_byte_lru_core_tracks_resident_bytes,
    crate_file,
};

#[test]
fn classified_token_memory_cache_is_byte_bounded() {
    let classified_memory =
        crate_file("src/parsing/c/preprocess/gpu_pipeline/cache/classified_memory.rs");
    assert!(
        classified_memory.contains("const CLASSIFIED_CACHE_MAX_BYTES: usize"),
        "Fix: classified-token memory cache must be byte-bounded, not only entry-bounded."
    );
    assert!(
        classified_memory.contains("ByteBoundLruCache<ClassifiedCacheKey, Arc<ClassifiedTokens>>"),
        "Fix: classified-token cache must use the shared byte-bounded LRU core instead of bespoke entry-only caching."
    );
    assert!(
        classified_memory.contains("classified_tokens_bytes(&value)"),
        "Fix: classified-token cache admission must size token columns and source bytes before storing."
    );
    assert_byte_lru_core_tracks_resident_bytes();
    assert_byte_lru_core_rejects_and_accounts();

    let classified_size = crate_file("src/parsing/c/preprocess/gpu_pipeline/classified_size.rs");
    assert!(
        classified_size.contains("pub(super) fn classified_tokens_bytes"),
        "Fix: classified-token byte sizing must live in one shared helper."
    );
    assert!(
        classified_size.contains("tok_types")
            && classified_size.contains("tok_starts")
            && classified_size.contains("tok_lens")
            && classified_size.contains("directive_kinds")
            && classified_size.contains("classified.source.len()"),
        "Fix: classified-token byte sizing must include every token column plus retained source bytes."
    );

    let header_reuse = crate_file("src/parsing/c/preprocess/gpu_pipeline/header_reuse.rs");
    assert!(
        header_reuse.contains("use super::classified_size::classified_tokens_bytes;"),
        "Fix: header reuse and classified-token cache must share the same classified-token sizing helper."
    );
}
