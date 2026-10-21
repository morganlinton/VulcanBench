//! Resource dos governance test suite.

const LEDGER: &str = include_str!("../../../docs/optimization/RESEARCH_SOURCE_LEDGER.toml");
const EVIDENCE: &str = include_str!("../../../docs/optimization/RESOURCE_EXHAUSTION_EVIDENCE.toml");
const COVERAGE: &str =
    include_str!("../../../docs/optimization/RESOURCE_DOS_GOVERNANCE_TRANCHE_COVERAGE.toml");

#[test]
fn resource_dos_primary_sources_are_registered() {
    for key in [
        "CWE_400_RESOURCE_CONSUMPTION",
        "CWE_770_RESOURCE_ALLOCATION",
        "CWE_407_ALGORITHMIC_COMPLEXITY",
        "CWE_674_UNCONTROLLED_RECURSION",
        "OWASP_DOS",
        "NIST_SP_800_190",
    ] {
        assert!(LEDGER.contains(key), "research source ledger must include {key}");
    }
}

#[test]
fn resource_exhaustion_evidence_links_triggers_limits_actions_errors_redaction_and_metrics() {
    for required in [
        "evidence_id",
        "trigger",
        "budget_id",
        "observed_usage",
        "limit",
        "action",
        "degraded_route",
        "operator_error",
        "redaction_state",
        "metrics_link",
    ] {
        assert!(
            EVIDENCE.contains(required),
            "resource exhaustion evidence must include {required}"
        );
    }
}

#[test]
fn resource_dos_tranche_coverage_preserves_shared_dedup_seams() {
    for required in [
        "VX-921..VX-940",
        "resource_budget_policy",
        "algorithmic_complexity_dos_guards",
        "backpressure_queue_quota_policy",
        "resource_exhaustion_evidence",
        "source_ledger_coverage",
        "publication_boundary",
        "dedup_seam",
        "proof_gate",
    ] {
        assert!(
            COVERAGE.contains(required),
            "resource DoS tranche coverage must include {required}"
        );
    }
}
