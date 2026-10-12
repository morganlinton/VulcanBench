//! Operator reporting interchange test suite.

const REPORTING: &str = include_str!("../../docs/optimization/OPERATOR_REPORTING_INTERCHANGE.toml");

#[test]
fn operator_reporting_interchange_requires_sarif_like_result_fields() {
    for required in [
        "result_id",
        "rule_id",
        "level",
        "message",
        "artifact_uri",
        "region",
        "fingerprints",
        "thread_flow",
        "fix",
        "redaction_state",
    ] {
        assert!(
            REPORTING.contains(required),
            "operator reporting interchange must include {required}"
        );
    }
}
