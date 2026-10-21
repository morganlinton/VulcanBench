//! Filesystem archive governance test suite.

const LEDGER: &str = include_str!("../../../docs/optimization/RESEARCH_SOURCE_LEDGER.toml");
const REDACTION: &str =
    include_str!("../../../docs/optimization/FILESYSTEM_EVIDENCE_REDACTION.toml");
const COVERAGE: &str =
    include_str!("../../../docs/optimization/FILESYSTEM_ARCHIVE_GOVERNANCE_TRANCHE_COVERAGE.toml");

#[test]
fn filesystem_archive_primary_sources_are_registered() {
    for key in [
        "OWASP_PATH_TRAVERSAL",
        "CWE_22_PATH_TRAVERSAL",
        "CWE_367_TOCTOU",
        "SEI_CERT_FIO45_C",
        "PKWARE_APPNOTE",
    ] {
        assert!(LEDGER.contains(key), "research source ledger must include {key}");
    }
}

#[test]
fn filesystem_evidence_redaction_links_paths_archives_errors_retention_and_reports() {
    for required in [
        "evidence_id",
        "raw_fields",
        "redacted_fields",
        "path_classification",
        "publication_class",
        "operator_error",
        "retention_policy",
        "report_link",
        "local-only-nonpublished-doc",
    ] {
        assert!(
            REDACTION.contains(required),
            "filesystem evidence redaction must include {required}"
        );
    }
}

#[test]
fn filesystem_archive_tranche_coverage_preserves_shared_dedup_seams() {
    for required in [
        "VX-901..VX-920",
        "path_canonicalization_policy",
        "archive_extraction_bomb_guards",
        "atomic_file_operation_race_policy",
        "filesystem_evidence_redaction",
        "source_ledger_coverage",
        "publication_boundary",
        "dedup_seam",
        "proof_gate",
    ] {
        assert!(
            COVERAGE.contains(required),
            "filesystem/archive tranche coverage must include {required}"
        );
    }
}
