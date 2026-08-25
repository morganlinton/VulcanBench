//! Cuda stream ordered pool planner test suite.

const PLANNER: &str = include_str!("../../docs/optimization/CUDA_STREAM_ORDERED_POOL_PLANNER.toml");

#[test]
fn cuda_stream_ordered_pool_planner_blocks_capture_allocations() {
    for required in [
        "staging",
        "scratch",
        "graph_capture",
        "resident_upload",
        "scan_database",
        "capture_allocation_allowed = false",
        "VYRE_CUDA_POOL_MISS_DURING_CAPTURE",
        "no-global-sync",
    ] {
        assert!(
            PLANNER.contains(required),
            "CUDA pool planner must include {required}"
        );
    }
}
