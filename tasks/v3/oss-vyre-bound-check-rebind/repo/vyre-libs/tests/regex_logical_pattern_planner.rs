//! Regex logical pattern planner test suite.

const PLANNER: &str =
    include_str!("../../docs/optimization/REGEX_LOGICAL_PATTERN_PLANNER.toml");

const REQUIRED_OPERATORS: &[&str] = &["and", "or", "not", "ordered_dependency"];
const REQUIRED_ROUTES: &[&str] = &[
    "separate_scans_plus_combine",
    "automata_product",
    "derivative_product",
];
const REQUIRED_METRICS: &[&str] = &[
    "output_parity",
    "state_growth",
    "active_ns",
    "dependency_semantics",
    "refusal_reason",
];

#[test]
fn regex_logical_pattern_planner_covers_operators_routes_and_metrics() {
    for operator in REQUIRED_OPERATORS {
        assert!(
            PLANNER.contains(&format!("operator_id = \"{operator}\"")),
            "Fix: logical regex planner must include operator `{operator}`"
        );
    }
    for route in REQUIRED_ROUTES {
        assert!(
            PLANNER.contains(&format!("route_id = \"{route}\"")),
            "Fix: logical regex planner must include route `{route}`"
        );
    }
    for metric in REQUIRED_METRICS {
        assert!(
            PLANNER.contains(&format!("\"{metric}\"")),
            "Fix: logical regex planner must require metric `{metric}`"
        );
    }
}

#[test]
fn regex_logical_pattern_planner_records_dependency_and_negative_semantics() {
    for required in [
        "dependency_semantics = \"all-child-patterns-match\"",
        "dependency_semantics = \"any-child-pattern-matches\"",
        "dependency_semantics = \"left-match-without-right-match\"",
        "dependency_semantics = \"child-patterns-match-in-declared-order\"",
        "requires_negative_evidence = true",
        "VYRE_SCAN_LOGICAL_NEGATIVE_EVIDENCE_REQUIRED",
    ] {
        assert!(
            PLANNER.contains(required),
            "Fix: logical regex planner must include `{required}`"
        );
    }
}

#[test]
fn regex_logical_pattern_planner_gates_state_growth_and_parity() {
    let route_rows = PLANNER.matches("[[route]]").count();
    assert_eq!(
        PLANNER.matches("parity_required = true").count(),
        route_rows,
        "Fix: every logical pattern route must require output parity"
    );
    assert!(
        PLANNER.contains("bounded-product-state-count")
            && PLANNER.contains("bounded-derivative-state-count")
            && PLANNER.contains("VYRE_SCAN_LOGICAL_STATE_GROWTH_EXCEEDED"),
        "Fix: logical regex planner must gate product-state growth"
    );
    assert_eq!(
        PLANNER
            .matches("evidence_path = \"vyre-libs/tests/regex_logical_pattern_planner.rs\"")
            .count(),
        PLANNER.matches("[[operator]]").count()
            + PLANNER.matches("[[route]]").count()
            + PLANNER.matches("[[refusal]]").count(),
        "Fix: every logical planner row must point at this proof gate"
    );
}
