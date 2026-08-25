//! Contract tests for GPU C preprocessor header-reuse cache budgets.

use std::fs;
use std::path::Path;

fn crate_file(path: &str) -> String {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    fs::read_to_string(manifest.join(path)).unwrap_or_else(|error| {
        panic!("failed to read {path}: {error}");
    })
}

fn header_reuse_source() -> String {
    crate_file("src/parsing/c/preprocess/gpu_pipeline/header_reuse.rs")
}

#[test]
fn header_reuse_cache_is_entry_and_byte_bounded() {
    let source = header_reuse_source();
    assert!(
        source.contains("const HEADER_REUSE_CACHE_MAX_BYTES: usize"),
        "Fix: header-analysis reuse must have an explicit byte budget, not only an entry count."
    );
    assert!(
        source.contains("ByteBoundLruCache<HeaderReuseKey, HeaderReuseEntry>"),
        "Fix: header-analysis cache must use the shared byte-bounded LRU core instead of bespoke entry-only caching."
    );
    assert!(
        source.contains("header_reuse_entry_bytes(&value)"),
        "Fix: cache insertion must size classified-token and directive-payload residency before storing."
    );
    let byte_lru_cache = crate_file("src/parsing/c/preprocess/gpu_pipeline/byte_lru_cache.rs");
    assert!(
        byte_lru_cache.contains("bytes: usize") && byte_lru_cache.contains("max_bytes: usize"),
        "Fix: shared GPU preprocessor cache core must track resident bytes and a byte limit."
    );
    assert!(
        byte_lru_cache.contains("entry_bytes > self.max_bytes"),
        "Fix: an oversized generated header must be rejected instead of pinning unbounded memory."
    );
    assert!(
        byte_lru_cache.contains("self.bytes") && byte_lru_cache.contains(".checked_add(entry_bytes)"),
        "Fix: shared GPU preprocessor cache core must check byte-budget admission before inserting entries."
    );
    assert!(
        source.contains("use super::classified_size::classified_tokens_bytes;")
            && source.contains("use super::payload_size::directive_payloads_bytes;"),
        "Fix: byte accounting must include classified token columns/source bytes and shared directive-payload dynamic sizing."
    );
    assert!(
        !source.contains(".lock().ok()?"),
        "Fix: poisoned header-reuse cache locks must be surfaced as errors, not converted into cache misses."
    );
    assert!(
        source.contains("header-analysis reuse cache poisoned"),
        "Fix: header-reuse cache lock failures need actionable error text."
    );

    let driver = fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/parsing/c/preprocess/gpu_pipeline/driver.rs"),
    )
    .unwrap_or_else(|error| {
        panic!("failed to read driver.rs: {error}");
    });
    assert!(
        driver.contains("defines_hash_cache: Option<(u64, [u8; 16])>"),
        "Fix: header-reuse keys must reuse a per-run live-defines hash instead of sorting/hashing every include."
    );
    assert!(
        driver.contains("fn live_defines_hash(&mut self) -> [u8; 16]")
            && driver.contains("fn invalidate_defines_hash(&mut self)"),
        "Fix: live-defines hash cache must have explicit lookup and invalidation paths."
    );

    let file_inputs = fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/parsing/c/preprocess/gpu_pipeline/driver/file_inputs.rs"),
    )
    .unwrap_or_else(|error| {
        panic!("failed to read file_inputs.rs: {error}");
    });
    assert!(
        file_inputs.contains("let defines_hash = run.live_defines_hash();")
            && file_inputs.contains("header_reuse_key_from_hash(")
            && file_inputs.contains("file_path,")
            && file_inputs.contains("source_hash,")
            && file_inputs.contains("defines_hash,"),
        "Fix: header-reuse key construction must consume the cached live-defines hash."
    );
    assert!(
        !file_inputs.contains("header_reuse_key(file_path, source, &run.macros)"),
        "Fix: include preparation must not rebuild the header-reuse defines hash from the full macro table on every include."
    );

    let macro_directives = fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/parsing/c/preprocess/gpu_pipeline/driver/macro_directives.rs"),
    )
    .unwrap_or_else(|error| {
        panic!("failed to read macro_directives.rs: {error}");
    });
    assert!(
        macro_directives
            .matches("run.invalidate_defines_hash();")
            .count()
            >= 2,
        "Fix: #define and #undef mutations must invalidate the cached live-defines hash."
    );
}
