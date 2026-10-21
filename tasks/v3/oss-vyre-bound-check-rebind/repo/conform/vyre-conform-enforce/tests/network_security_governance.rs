//! Network security governance test suite.

const LEDGER: &str = include_str!("../../../docs/optimization/RESEARCH_SOURCE_LEDGER.toml");
const REDACTION: &str =
    include_str!("../../../docs/optimization/NETWORK_EVIDENCE_REDACTION.toml");
const COVERAGE: &str =
    include_str!("../../../docs/optimization/NETWORK_SECURITY_GOVERNANCE_TRANCHE_COVERAGE.toml");

#[test]
fn network_security_primary_sources_are_registered() {
    for key in ["WHATWG_URL", "RFC_9110_HTTP", "OWASP_SSRF", "RFC_1035_DNS"] {
        assert!(LEDGER.contains(key), "research source ledger must include {key}");
    }
}

#[test]
fn network_evidence_redaction_links_raw_redacted_fields_publication_traces_errors_and_retention() {
    for required in [
        "evidence_id",
        "raw_fields",
        "redacted_fields",
        "secret_detection",
        "publication_class",
        "trace_link",
        "operator_error",
        "retention_policy",
        "local-only-nonpublished-doc",
        "public-vyre-artifact",
    ] {
        assert!(
            REDACTION.contains(required),
            "network evidence redaction must include {required}"
        );
    }
}

#[test]
fn network_security_tranche_coverage_preserves_shared_dedup_seams() {
    for required in [
        "VX-861..VX-880",
        "url_canonicalization_policy",
        "ssrf_dns_rebinding_guards",
        "http_proxy_redirect_policy",
        "network_evidence_redaction",
        "source_ledger_coverage",
        "publication_boundary",
        "dedup_seam",
        "proof_gate",
    ] {
        assert!(
            COVERAGE.contains(required),
            "network security tranche coverage must include {required}"
        );
    }
}
