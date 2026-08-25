//! Extraction memory verifier cost model test suite.

const COST_MODEL: &str = include_str!("../../docs/optimization/EXTRACTION_MEMORY_VERIFIER_COST_MODEL.toml");

#[test]
fn extraction_cost_model_records_memory_and_verifier_terms() {
    for required in [
        "transfer_bytes",
        "coalescing_class",
        "candidate_count",
        "branch_divergence_proxy",
        "measured_counter_ids",
        "verifier_work_units",
        "route_change_reason",
    ] {
        assert!(
            COST_MODEL.contains(required),
            "extraction cost model must include {required}"
        );
    }
}
