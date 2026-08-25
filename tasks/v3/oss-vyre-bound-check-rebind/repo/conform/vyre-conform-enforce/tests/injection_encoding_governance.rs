//! Injection encoding governance test suite.

const LEDGER: &str = include_str!("../../../docs/optimization/RESEARCH_SOURCE_LEDGER.toml");
const COVERAGE: &str =
    include_str!("../../../docs/optimization/INJECTION_ENCODING_GOVERNANCE_TRANCHE_COVERAGE.toml");

#[test]
fn injection_encoding_primary_sources_are_registered() {
    for key in [
        "OWASP_OS_COMMAND_INJECTION",
        "CWE_78_OS_COMMAND_INJECTION",
        "CWE_93_CRLF_INJECTION",
        "CWE_117_LOG_NEUTRALIZATION",
        "RFC_8259_JSON",
        "UNICODE_TR36",
        "UNICODE_UTS39",
    ] {
        assert!(LEDGER.contains(key), "research source ledger must include {key}");
    }
}

#[test]
fn injection_encoding_tranche_coverage_preserves_shared_dedup_seams() {
    for required in [
        "VX-941..VX-960",
        "command_execution_boundary",
        "control_character_output_policy",
        "structured_output_encoding_policy",
        "unicode_identifier_spoofing_policy",
        "source_ledger_coverage",
        "publication_boundary",
        "dedup_seam",
        "proof_gate",
    ] {
        assert!(
            COVERAGE.contains(required),
            "injection encoding tranche coverage must include {required}"
        );
    }
}
