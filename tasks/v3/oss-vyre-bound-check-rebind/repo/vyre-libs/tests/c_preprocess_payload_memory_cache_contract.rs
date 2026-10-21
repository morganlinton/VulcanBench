//! Structural contracts for C preprocessing payload memory caching.

use std::fs;
use std::path::Path;

fn crate_file(path: &str) -> String {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    fs::read_to_string(manifest.join(path)).unwrap_or_else(|error| {
        panic!("failed to read {path}: {error}");
    })
}

#[test]
fn directive_payloads_use_process_resident_memory_before_disk_cache() {
    let cache_mod = crate_file("src/parsing/c/preprocess/gpu_pipeline/cache.rs");
    assert!(
        cache_mod.contains("#[path = \"cache/payload_memory.rs\"]\nmod payload_memory;"),
        "Fix: directive payload memory residency must live in its own cache module, not inside disk persistence or the driver."
    );
    assert!(
        cache_mod.contains("pub(super) use payload_memory::{cached_payloads, insert_payloads};"),
        "Fix: production preprocessing must expose payload memory-cache lookup/insert helpers."
    );
    assert!(
        cache_mod.contains("#[path = \"cache/payload_memory.rs\"]"),
        "Fix: payload memory residency must remain separate from disk-cache persistence."
    );

    let payload_memory =
        crate_file("src/parsing/c/preprocess/gpu_pipeline/cache/payload_memory.rs");
    assert!(
        payload_memory.contains("const PAYLOAD_CACHE_MAX_BYTES: usize"),
        "Fix: directive payload memory cache must be byte-bounded, not only entry-bounded."
    );
    assert!(
        payload_memory.contains("ByteBoundLruCache<PayloadsCacheKey, Arc<[DirectivePayload]>>"),
        "Fix: directive payload memory cache must use the shared byte-bounded LRU core instead of bespoke entry-only caching."
    );
    assert!(
        payload_memory.contains("directive_payloads_bytes(&value)"),
        "Fix: directive payload cache admission must size dynamic payload bodies before storing."
    );
    let byte_lru_cache = crate_file("src/parsing/c/preprocess/gpu_pipeline/byte_lru_cache.rs");
    assert!(
        byte_lru_cache.contains("bytes: usize") && byte_lru_cache.contains("max_bytes: usize"),
        "Fix: shared GPU preprocessor cache core must track resident bytes and a byte limit."
    );
    assert!(
        byte_lru_cache.contains("entry_bytes > self.max_bytes")
            && byte_lru_cache.contains(".checked_add(entry_bytes)"),
        "Fix: shared GPU preprocessor cache core must reject oversized entries and evict to byte budget."
    );

    let file_inputs = crate_file("src/parsing/c/preprocess/gpu_pipeline/driver/file_inputs.rs");
    let memory_lookup = file_inputs.find("cached_payloads(&payloads_key)?").expect(
        "Fix: check process-resident directive payload cache before disk or GPU extraction.",
    );
    let disk_lookup = file_inputs
        .find("load_payloads_from_disk(&payloads_key)")
        .expect("Fix: disk payload cache remains the second-tier fallback after memory.");
    let gpu_extract = file_inputs
        .find("gpu_extract_directive_payloads_for_driver_with_scratch")
        .expect("Fix: GPU directive payload extraction remains the final miss path.");
    assert!(
        memory_lookup < disk_lookup && disk_lookup < gpu_extract,
        "Fix: payload lookup order must be memory cache -> disk cache -> GPU extraction."
    );

    let insert_count = file_inputs.matches("insert_payloads(").count();
    assert!(
        insert_count >= 2,
        "Fix: payload memory cache must be populated after disk hits and after fresh GPU extraction."
    );

    let payload_size = crate_file("src/parsing/c/preprocess/gpu_pipeline/payload_size.rs");
    assert!(
        payload_size.contains("pub(super) fn directive_payloads_bytes"),
        "Fix: directive payload byte sizing must live in one shared helper, not duplicated across caches."
    );
}
