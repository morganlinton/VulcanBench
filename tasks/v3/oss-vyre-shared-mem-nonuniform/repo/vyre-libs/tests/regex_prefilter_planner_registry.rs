//! Regex prefilter planner registry test suite.

const REGISTRY: &str = include_str!("../../docs/optimization/REGEX_PREFILTER_PLANNER.toml");

const REQUIRED_ROUTES: &[&str] = &[
    "exact_automata",
    "literal_prefilter_plus_verifier",
    "broader_prefilter_plus_verifier",
];

const REQUIRED_METRICS: &[&str] = &[
    "candidate_count",
    "verifier_work_units",
    "match_parity",
    "active_ns",
    "unsupported_reason",
];

#[test]
fn regex_prefilter_planner_registry_covers_required_routes_and_metrics() {
    for route in REQUIRED_ROUTES {
        assert!(
            REGISTRY.contains(&format!("route_id = \"{route}\"")),
            "Fix: regex prefilter planner registry must include route `{route}`"
        );
    }
    for metric in REQUIRED_METRICS {
        assert!(
            REGISTRY.contains(&format!("\"{metric}\"")),
            "Fix: regex prefilter planner registry must require metric `{metric}`"
        );
    }
}

#[test]
fn regex_prefilter_planner_registry_requires_verifier_for_approximation() {
    assert!(
        REGISTRY.contains("route_id = \"broader_prefilter_plus_verifier\"")
            && REGISTRY.contains("exactness = \"approximate-prefilter-verified\"")
            && REGISTRY.contains("verifier_required = true")
            && REGISTRY.contains("match_parity_required = true"),
        "Fix: broader prefilter approximation must require verifier and match parity"
    );
    assert!(
        REGISTRY.contains("diagnostic_code = \"VYRE_SCAN_APPROXIMATION_REQUIRES_VERIFIER\""),
        "Fix: approximate prefilter planner must reject missing verifier evidence"
    );
}

#[test]
fn regex_prefilter_planner_registry_records_comparators_and_rejections() {
    let route_rows = REGISTRY.matches("[[route]]").count();
    assert_eq!(
        route_rows,
        REQUIRED_ROUTES.len(),
        "Fix: regex prefilter planner must have one row per required route"
    );
    assert_eq!(
        REGISTRY.matches("evidence_path = \"vyre-libs/tests/regex_prefilter_planner_registry.rs\"").count(),
        route_rows + REGISTRY.matches("[[rejection]]").count(),
        "Fix: every prefilter route and rejection row must point at this proof gate"
    );
    for required in [
        "comparator = \"exact_automata\"",
        "candidate_policy = \"literal-factor-candidates\"",
        "candidate_policy = \"broader-construct-candidates\"",
        "VYRE_SCAN_PREFILTER_CANDIDATE_SATURATION",
        "VYRE_SCAN_PREFILTER_PARITY_MISSING",
    ] {
        assert!(
            REGISTRY.contains(required),
            "Fix: regex prefilter planner registry must include `{required}`"
        );
    }
}
