//! Property differential oracles test suite.

const ORACLES: &str = include_str!("../../docs/optimization/PROPERTY_DIFFERENTIAL_ORACLES.toml");

#[test]
fn property_differential_oracles_require_generators_shrinkers_lanes_and_reducers() {
    for required in [
        "oracle_id",
        "property",
        "generator",
        "shrinker",
        "differential_lanes",
        "validity_predicate",
        "reducer",
        "minimal_counterexample_digest",
        "scan-roundtrip-property",
        "c-frontend-differential",
        "graph-csr-algebra-property",
    ] {
        assert!(
            ORACLES.contains(required),
            "property differential oracle registry must include {required}"
        );
    }
}
