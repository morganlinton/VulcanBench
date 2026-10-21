//! Source contracts for C GPU-preprocess live-conditional staging allocation.

use std::fs;
use std::path::PathBuf;

fn src_path(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(relative)
}

fn read_src(relative: &str) -> String {
    fs::read_to_string(src_path(relative)).unwrap_or_else(|err| {
        panic!("failed to read {relative}: {err}");
    })
}

#[test]
fn live_conditional_staging_uses_checked_fallible_allocation_paths() {
    let live_state = read_src("src/parsing/c/preprocess/gpu_pipeline/live_state.rs");
    assert!(
        live_state.contains("fn reserve_live_vec<T>(")
            && live_state.contains("fn live_word_bytes("),
        "live conditional staging must centralize fallible reservation and checked byte sizing"
    );
    assert!(
        live_state.contains("seen_names.try_reserve(macros.len())"),
        "live macro-name dedupe must reserve fallibly"
    );
    assert!(
        live_state.contains("let batch_source_bytes = rows.iter().try_fold"),
        "batched live ifdef staging must precompute source bytes with overflow checks"
    );
    assert!(
        !live_state.contains("Vec::with_capacity(total_name_bytes)")
            && !live_state.contains("Vec::with_capacity(macros.len() + 1)")
            && !live_state.contains("seen_names.reserve(macros.len())")
            && !live_state.contains("scratch.batch_row_starts.reserve(row_count_bucket)")
            && !live_state.contains("scratch.batch_row_lens.reserve(row_count_bucket)")
            && !live_state.contains("scratch.batch_directive_kinds.reserve(row_count_bucket)")
            && !live_state.contains("scratch.out_scalar.resize(row_count_bucket * 4, 0)"),
        "live conditional staging must not use unchecked reserve or byte multiplication"
    );
}
