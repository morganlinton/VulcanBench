//! Pass research trace artifacts test suite.

const TRACES: &str = include_str!("../../docs/optimization/PASS_RESEARCH_TRACE_ARTIFACTS.toml");

#[test]
fn pass_research_traces_record_selected_rejected_and_skipped_evidence() {
    for required in [
        "source_key",
        "hypothesis",
        "comparator",
        "measured_metric",
        "falsification_result",
        "selected_pass_decision",
        "selected",
        "rejected",
        "skipped",
    ] {
        assert!(
            TRACES.contains(required),
            "pass research trace artifact must include {required}"
        );
    }
}
