//! Contract tests for the GPU C preprocessor macro-rescan fast path.

use std::fs;

#[test]
fn recursive_macro_rescan_uses_boolean_live_prefilter_before_table_clone() {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let flush = fs::read_to_string(format!(
        "{manifest_dir}/src/parsing/c/preprocess/gpu_pipeline/macro_expansion/flush.rs"
    ))
    .expect("macro expansion flush source must be readable");
    let segments = fs::read_to_string(format!(
        "{manifest_dir}/src/parsing/c/preprocess/gpu_pipeline/segments.rs"
    ))
    .expect("macro segment source must be readable");

    assert!(
        segments.contains("pub(super) fn has_live_macro_for_segment_excluding"),
        "Fix: recursive macro rescans need a boolean live-use prefilter so they do not clone macro definitions before proving a rescan is useful."
    );
    assert!(
        flush.contains("has_live_macro_for_segment_excluding("),
        "Fix: macro expansion rescan must use the boolean live-use prefilter before cloning a filtered macro table."
    );
    assert!(
        !flush.contains("let rescan_live_macros"),
        "Fix: recursive macro rescan must not allocate a cloned live macro Vec just to decide whether recursion is needed."
    );
}
