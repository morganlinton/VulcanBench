//! Frontier load balancing policies test suite.

const POLICIES: &str = include_str!("../../docs/optimization/FRONTIER_LOAD_BALANCING_POLICIES.toml");

#[test]
fn frontier_policies_record_imbalance_and_route_reasons() {
    for required in [
        "advance",
        "filter",
        "two_phase",
        "work_stealing",
        "frontier_size",
        "degree_skew",
        "imbalance_counter",
        "selected_policy",
        "traversal_digest",
    ] {
        assert!(
            POLICIES.contains(required),
            "frontier policy registry must include {required}"
        );
    }
}
