//! Structural contracts for C preprocessing replacement-token caching.

mod support;

use support::{
    assert_byte_lru_core_rejects_and_accounts, assert_byte_lru_core_tracks_resident_bytes,
    crate_file,
};

#[test]
fn replacement_token_cache_is_entry_and_byte_bounded() {
    let model = crate_file("src/parsing/c/preprocess/gpu_pipeline/token_provenance/model.rs");
    assert!(
        model.contains("REPLACEMENT_TOKEN_CACHE_MAX_BYTES"),
        "Fix: replacement-token cache must expose an explicit byte budget."
    );

    let cache =
        crate_file("src/parsing/c/preprocess/gpu_pipeline/token_provenance/replacement_cache.rs");
    assert!(
        cache.contains("use crate::parsing::c::preprocess::gpu_pipeline::classified_size::classified_tokens_bytes;"),
        "Fix: replacement-token cache must share classified-token byte sizing with other preprocessor caches."
    );
    assert!(
        cache.contains(
            "ByteBoundLruCache<ReplacementTokenCacheKey, std::sync::Arc<ClassifiedTokens>>"
        ),
        "Fix: replacement-token cache must use the shared byte-bounded LRU core instead of bespoke entry-only caching."
    );
    assert!(
        cache.contains("classified_tokens_bytes(&value)"),
        "Fix: replacement-token cache admission must size retained classified tokens before storing."
    );
    assert_byte_lru_core_tracks_resident_bytes();
    assert_byte_lru_core_rejects_and_accounts();
}
