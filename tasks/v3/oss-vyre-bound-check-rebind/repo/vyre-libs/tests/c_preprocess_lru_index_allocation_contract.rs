//! Source contracts for C GPU-preprocess LRU index compaction.

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
fn lru_index_compaction_rebuilds_transactionally_with_fallible_reserve() {
    let lru = read_src("src/parsing/c/preprocess/gpu_pipeline/lru_index.rs");
    assert!(
        lru.contains("live_entries.checked_mul(4)")
            && lru.contains("let mut compacted = BinaryHeap::new();")
            && lru.contains("compacted.try_reserve(live_entries)")
            && lru.contains("self.heap = compacted;"),
        "LRU compaction must compute thresholds explicitly and rebuild into fallible scratch before replacing the live heap"
    );
    assert!(
        !lru.contains("live_entries.saturating_mul(4)")
            && !lru.contains("self.heap.clear();")
            && !lru.contains("self.heap.reserve(live_entries)"),
        "LRU compaction must not silently saturate or clear the live heap before allocation succeeds"
    );
}
