//! Adversarial validation governance test suite.

const LEDGER: &str = include_str!("../../../docs/optimization/RESEARCH_SOURCE_LEDGER.toml");
const VULN: &str = include_str!("../../../docs/optimization/VULNERABILITY_REPLAY_MAPPING.toml");
const COVERAGE: &str =
    include_str!("../../../docs/optimization/ADVERSARIAL_VALIDATION_TRANCHE_COVERAGE.toml");

#[test]
fn adversarial_validation_primary_sources_are_registered() {
    for key in [
        "LLVM_LIBFUZZER",
        "AFLPLUSPLUS",
        "CARGO_FUZZ",
        "OSS_FUZZ",
        "LLVM_SANITIZER_COVERAGE",
        "LLVM_SOURCE_COVERAGE",
        "OSV_SCHEMA",
        "CWE",
    ] {
        assert!(LEDGER.contains(key), "research source ledger must include {key}");
    }
}

#[test]
fn vulnerability_replay_mapping_connects_osv_cwe_fixtures_findings_and_remediation() {
    for required in [
        "advisory_id",
        "package_or_component",
        "affected_range",
        "fixed_range",
        "cwe_id",
        "fixture_id",
        "finding_rule_id",
        "expected_level",
        "remediation",
    ] {
        assert!(
            VULN.contains(required),
            "vulnerability replay mapping must include {required}"
        );
    }
}

#[test]
fn adversarial_validation_tranche_coverage_preserves_shared_dedup_seams() {
    for required in [
        "VX-741..VX-760",
        "fuzz_inventory",
        "corpus_minimization",
        "vulnerability_replay",
        "coverage_sanitizer_matrix",
        "operator_reporting_link",
        "trace_metrics_link",
        "dedup_seam",
        "proof_gate",
    ] {
        assert!(
            COVERAGE.contains(required),
            "adversarial validation coverage must include {required}"
        );
    }
}
