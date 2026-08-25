//! Benchmark hygiene contracts for separating setup timing from steady-state release metrics.

#[test]
fn release_bench_does_not_present_setup_timing_as_steady_state_performance() {
    let src = include_str!("../benches/release.rs");

    assert!(
        !src.contains("release_registry_inventory_collection"),
        "Fix: registry inventory collection is setup timing, not a release steady-state benchmark"
    );
    assert!(
        src.contains("cold_setup_registry_inventory_collection"),
        "Fix: setup-only benchmarks must be explicitly quarantined with a cold_setup prefix"
    );
    assert!(
        !src.contains("bench_function(\"release_") || src.contains("b.iter(||"),
        "Fix: release benchmark names must not wrap setup/cold-path collection without an explicit steady-state loop"
    );
}
