//! Integration test crate for the containing Vyre package.

use std::fs;
use std::path::Path;

fn read(path: &str) -> String {
    fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(path))
        .unwrap_or_else(|error| panic!("failed to read {path}: {error}"))
}

#[test]
fn compiled_pipeline_caches_are_entry_and_byte_bounded() {
    let cache_utils = read("src/pipeline/backend_select/cache_utils.rs");
    assert!(cache_utils.contains("bytes: usize"));
    assert!(cache_utils.contains("insert_with_cost"));
    assert!(cache_utils.contains("entry_bytes > max_bytes"));
    assert!(cache_utils.contains("self.bytes > max_bytes"));
    // The running byte total must accumulate through an OVERFLOW-SAFE add, never
    // a raw `+`/`+=` that could wrap and defeat the budget. The implementation
    // uses `saturating_add`: on the (physically unreachable, ~2^64-byte) overflow
    // it pins to usize::MAX, which immediately trips the `self.bytes > max_bytes`
    // eviction loop above — a complete bound, equivalent in safety to a checked
    // add with an explicit reject branch.
    assert!(cache_utils.contains("saturating_add(entry_bytes)"));

    let backend_select = read("src/pipeline/backend_select.rs");
    assert!(backend_select.contains("COMPILED_PIPELINE_CACHE_MAX_ENTRIES"));
    assert!(backend_select.contains("COMPILED_PIPELINE_CACHE_MAX_BYTES"));
    assert!(backend_select.contains("compiled_pipeline_cache_estimated_bytes"));
    assert!(backend_select.contains("program.stats()"));
    assert!(backend_select.contains("node_count"));

    let borrowed_cache = read("src/pipeline/backend_select/borrowed_cache.rs");
    assert!(borrowed_cache.contains("compiled_pipeline_cache_estimated_bytes(program)"));
    assert!(borrowed_cache.contains("compiled_pipeline_cache_estimated_bytes(&program)"));
    assert_eq!(borrowed_cache.matches("insert_with_cost").count(), 2);
    assert_eq!(
        borrowed_cache
            .matches("COMPILED_PIPELINE_CACHE_MAX_BYTES")
            .count(),
        2
    );

    let resident_dispatch = read("src/pipeline/backend_select/resident_dispatch.rs");
    assert_eq!(
        resident_dispatch
            .matches("compiled_pipeline_cache_estimated_bytes(&program)")
            .count(),
        1
    );
    assert_eq!(resident_dispatch.matches("insert_with_cost").count(), 1);
    assert_eq!(
        resident_dispatch
            .matches("COMPILED_PIPELINE_CACHE_MAX_BYTES")
            .count(),
        1
    );
    assert!(
        resident_dispatch
            .matches("resident_cached_pipeline(")
            .count()
            >= 2
    );
}
