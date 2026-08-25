//! Flow precision planner test suite.

const PLANNER: &str = include_str!("../../docs/optimization/FLOW_PRECISION_PLANNER.toml");

#[test]
fn flow_precision_planner_records_costed_modes_and_witnesses() {
    for required in [
        "local",
        "summary",
        "global",
        "taint",
        "witness_reconstruction",
        "sanitizer_policy",
        "cost_budget",
        "route_reason",
        "witness_requirement",
    ] {
        assert!(
            PLANNER.contains(required),
            "flow precision planner must include {required}"
        );
    }

    assert!(PLANNER.contains("local-intraprocedural-sanitized"));
    assert!(PLANNER.contains("global-taint-with-witness"));
}
