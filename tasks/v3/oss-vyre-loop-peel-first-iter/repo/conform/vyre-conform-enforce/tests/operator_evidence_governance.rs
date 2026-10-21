//! Operator evidence governance test suite.

const LEDGER: &str = include_str!("../../../docs/optimization/RESEARCH_SOURCE_LEDGER.toml");
const CORRELATION: &str =
    include_str!("../../../docs/optimization/CROSS_SIGNAL_AUDIT_CORRELATION.toml");
const COVERAGE: &str =
    include_str!("../../../docs/optimization/OPERATOR_EVIDENCE_TRANCHE_COVERAGE.toml");

#[test]
fn operator_evidence_primary_sources_are_in_the_research_ledger() {
    for key in [
        "SARIF_2_1_0",
        "OPENTELEMETRY_SEMCONV",
        "W3C_TRACE_CONTEXT",
        "OPENMETRICS_PROMETHEUS",
    ] {
        assert!(LEDGER.contains(key), "research source ledger must include {key}");
    }
}

#[test]
fn cross_signal_audit_correlation_joins_findings_traces_metrics_and_provenance() {
    for required in [
        "correlation_id",
        "finding_result_id",
        "trace_id",
        "metric_name",
        "provenance_artifact",
        "reproducibility_capsule",
        "source_impact_edge",
        "redaction_state",
    ] {
        assert!(
            CORRELATION.contains(required),
            "cross-signal audit correlation must include {required}"
        );
    }
}

#[test]
fn operator_evidence_tranche_coverage_preserves_dedup_seams() {
    for required in [
        "VX-721..VX-740",
        "reporting_contract",
        "trace_contract",
        "metrics_contract",
        "cross_signal_contract",
        "source_ledger_coverage",
        "dedup_seam",
        "proof_gate",
    ] {
        assert!(
            COVERAGE.contains(required),
            "operator evidence tranche coverage must include {required}"
        );
    }
}
