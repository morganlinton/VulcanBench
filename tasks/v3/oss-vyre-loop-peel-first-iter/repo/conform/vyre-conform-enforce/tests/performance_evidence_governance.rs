//! Performance evidence governance test suite.

const LEDGER: &str = include_str!("../../../docs/optimization/RESEARCH_SOURCE_LEDGER.toml");
const PUBLICATION: &str =
    include_str!("../../../docs/optimization/PERFORMANCE_ARTIFACT_PUBLICATION.toml");
const COVERAGE: &str =
    include_str!("../../../docs/optimization/PERFORMANCE_VALIDATION_TRANCHE_COVERAGE.toml");

#[test]
fn performance_primary_sources_are_registered() {
    for key in [
        "CRITERION_RS",
        "GOOGLE_BENCHMARK",
        "ROOFLINE_MODEL",
        "NSIGHT_COMPUTE_ROOFLINE",
    ] {
        assert!(LEDGER.contains(key), "research source ledger must include {key}");
    }
}

#[test]
fn performance_artifact_publication_requires_methodology_stats_roofline_and_privacy() {
    for required in [
        "artifact_id",
        "benchmark_id",
        "methodology_contract",
        "regression_gate",
        "roofline_evidence",
        "reproducibility_capsule",
        "source_ledger_keys",
        "publication_class",
        "private_path_scan",
        "publication_allowed",
        "public-vyre-artifact",
        "local-only-nonpublished-doc",
    ] {
        assert!(
            PUBLICATION.contains(required),
            "performance artifact publication must include {required}"
        );
    }
}

#[test]
fn performance_validation_tranche_coverage_preserves_shared_dedup_seams() {
    for required in [
        "VX-781..VX-800",
        "benchmark_methodology",
        "statistical_regression_gate",
        "roofline_counter_evidence",
        "performance_artifact_publication",
        "source_ledger_coverage",
        "publication_boundary",
        "dedup_seam",
        "proof_gate",
    ] {
        assert!(
            COVERAGE.contains(required),
            "performance validation coverage must include {required}"
        );
    }
}
