//! Data lineage governance test suite.

const LEDGER: &str = include_str!("../../../docs/optimization/RESEARCH_SOURCE_LEDGER.toml");
const MANIFEST: &str =
    include_str!("../../../docs/optimization/DATASET_PUBLICATION_MANIFEST.toml");
const COVERAGE: &str =
    include_str!("../../../docs/optimization/DATA_LINEAGE_GOVERNANCE_TRANCHE_COVERAGE.toml");

#[test]
fn data_lineage_primary_sources_are_registered() {
    for key in [
        "W3C_PROV_O",
        "W3C_DCAT_3",
        "NIST_PRIVACY_FRAMEWORK",
        "NIST_SP_800_53",
    ] {
        assert!(LEDGER.contains(key), "research source ledger must include {key}");
    }
}

#[test]
fn dataset_publication_manifest_links_lineage_privacy_retention_boundary_and_release_artifact() {
    for required in [
        "manifest_id",
        "dataset_id",
        "lineage_contract",
        "privacy_classification",
        "retention_control",
        "publication_boundary",
        "release_artifact",
        "publication_allowed",
        "private-local-debug-blocked",
    ] {
        assert!(
            MANIFEST.contains(required),
            "dataset publication manifest must include {required}"
        );
    }
}

#[test]
fn data_lineage_tranche_coverage_preserves_shared_dedup_seams() {
    for required in [
        "VX-841..VX-860",
        "dataset_lineage_catalog",
        "corpus_privacy_classification",
        "data_retention_media_controls",
        "dataset_publication_manifest",
        "source_ledger_coverage",
        "publication_boundary",
        "dedup_seam",
        "proof_gate",
    ] {
        assert!(
            COVERAGE.contains(required),
            "data lineage tranche coverage must include {required}"
        );
    }
}
