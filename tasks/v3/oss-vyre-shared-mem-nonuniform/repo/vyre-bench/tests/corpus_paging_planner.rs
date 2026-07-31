//! Corpus paging planner test suite.

const PLANNER: &str = include_str!("../../docs/optimization/CORPUS_PAGING_PLANNER.toml");

#[test]
fn corpus_paging_planner_records_routes_counters_and_blockers() {
    for required in [
        "mmap_cpu",
        "cpu_staging",
        "async_prefetch",
        "direct_gpu_dma",
        "transfer_counter",
        "match_or_recall_parity",
        "capability_blocker",
        "residency_budget",
    ] {
        assert!(
            PLANNER.contains(required),
            "corpus paging planner must include {required}"
        );
    }
}
