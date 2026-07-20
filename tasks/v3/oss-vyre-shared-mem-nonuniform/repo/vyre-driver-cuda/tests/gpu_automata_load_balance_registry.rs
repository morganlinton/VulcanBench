//! Gpu automata load balance registry test suite.

const REGISTRY: &str =
    include_str!("../../docs/optimization/GPU_AUTOMATA_LOAD_BALANCE.toml");

#[test]
fn gpu_automata_load_balance_registry_records_required_fields() {
    for required in [
        "state_fanout",
        "row_length_distribution",
        "failureless_depth",
        "work_queue_spill",
        "match_parity",
        "degenerate_prefix_fanout",
        "balanced_literal_set",
        "adversarial_overlap_density",
    ] {
        assert!(
            REGISTRY.contains(required),
            "Fix: GPU automata load-balance registry must include `{required}`"
        );
    }
}

#[test]
fn gpu_automata_load_balance_registry_requires_parity_and_spill_evidence() {
    assert_eq!(
        REGISTRY.matches("match_parity = \"required\"").count(),
        REGISTRY.matches("[[case]]").count(),
        "Fix: every GPU automata load-balance case must require match parity"
    );
    for required in [
        "work_queue_spill = \"required\"",
        "work_queue_spill = \"measured\"",
        "row_length_distribution = \"skewed\"",
    ] {
        assert!(
            REGISTRY.contains(required),
            "Fix: GPU automata load-balance registry must include `{required}`"
        );
    }
    assert_eq!(
        REGISTRY
            .matches("evidence_path = \"vyre-driver-cuda/tests/gpu_automata_load_balance_registry.rs\"")
            .count(),
        REGISTRY.matches("[[case]]").count(),
        "Fix: every GPU automata load-balance row must point at this proof gate"
    );
}
