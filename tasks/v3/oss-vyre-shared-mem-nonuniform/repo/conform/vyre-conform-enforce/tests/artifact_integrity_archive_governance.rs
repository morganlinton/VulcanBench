//! Artifact integrity archive governance test suite.

const LEDGER: &str = include_str!("../../../docs/optimization/RESEARCH_SOURCE_LEDGER.toml");
const PLAN: &str = include_str!("../../../docs/optimization/ALL_AXES_ACCELERATION_PLAN.md");
const INTEGRITY: &str =
    include_str!("../../../docs/optimization/RELEASE_ARTIFACT_INTEGRITY_INDEX.toml");
const VERIFY: &str =
    include_str!("../../../docs/optimization/CONSUMER_ARTIFACT_VERIFICATION_PROTOCOL.toml");
const ROLLBACK: &str =
    include_str!("../../../docs/optimization/RELEASE_METADATA_ANTI_ROLLBACK_POLICY.toml");
const ARCHIVE: &str =
    include_str!("../../../docs/optimization/SOURCE_ARCHIVE_DURABILITY_MAP.toml");
const COVERAGE: &str = include_str!(
    "../../../docs/optimization/END_TO_END_ARTIFACT_INTEGRITY_ARCHIVE_TRANCHE_COVERAGE.toml"
);

#[test]
fn artifact_integrity_archive_sources_are_registered() {
    for key in [
        "GITHUB_IMMUTABLE_RELEASES",
        "CARGO_REGISTRY_INDEX_CHECKSUM",
        "SIGSTORE_COSIGN_VERIFY",
        "SLSA_VERIFIER",
        "TUF_SPECIFICATION",
        "SOFTWARE_HERITAGE_DEPOSIT",
    ] {
        assert!(LEDGER.contains(key), "research source ledger must include {key}");
    }
}

#[test]
fn release_artifact_integrity_index_records_immutable_identities_digests_signatures_attestations_sboms_and_boundaries() {
    for required in [
        "artifact_id",
        "artifact_surface",
        "immutable_identity",
        "digest_policy",
        "signature_policy",
        "attestation_policy",
        "sbom_policy",
        "transparency_policy",
        "source_archive_policy",
        "publication_boundary",
        "vyre-crates-io-package",
        "vyre-github-release-asset",
        "vyre-release-metadata-bundle",
        "redacted-release-evidence-summary",
    ] {
        assert!(
            INTEGRITY.contains(required),
            "release artifact integrity index must include {required}"
        );
    }
}

#[test]
fn consumer_artifact_verification_protocol_records_trust_roots_fetch_digest_signature_source_failure_and_offline_policies() {
    for required in [
        "verifier_id",
        "consumer_surface",
        "trust_root_policy",
        "fetch_policy",
        "digest_check",
        "signature_attestation_check",
        "source_expectation_check",
        "failure_policy",
        "offline_policy",
        "cargo-package-consumer",
        "github-release-asset-consumer",
        "enterprise-mirror-consumer",
    ] {
        assert!(
            VERIFY.contains(required),
            "consumer artifact verification protocol must include {required}"
        );
    }
}

#[test]
fn release_metadata_anti_rollback_policy_records_tuf_style_threshold_expiration_rollback_freeze_and_mirror_controls() {
    for required in [
        "metadata_id",
        "root_trust_policy",
        "target_metadata_policy",
        "snapshot_policy",
        "timestamp_policy",
        "threshold_policy",
        "expiration_policy",
        "rollback_freeze_policy",
        "mirror_policy",
        "public-release-update-metadata",
        "airgapped-consumer-bundle",
    ] {
        assert!(
            ROLLBACK.contains(required),
            "release metadata anti rollback policy must include {required}"
        );
    }
}

#[test]
fn source_archive_durability_map_links_public_source_tags_crate_archives_swhids_checksums_retention_and_private_exclusion() {
    for required in [
        "archive_id",
        "source_surface",
        "persistent_identifier",
        "source_tag_policy",
        "package_archive_policy",
        "swhid_policy",
        "crate_checksum_policy",
        "retention_policy",
        "private_boundary_policy",
        "public-vyre-source-tag",
        "public-crate-package-archive",
        "private-local-evidence-exclusion",
    ] {
        assert!(
            ARCHIVE.contains(required),
            "source archive durability map must include {required}"
        );
    }
}

#[test]
fn plan_contains_artifact_integrity_archive_rows() {
    for row in [
        "VX-1161",
        "VX-1162",
        "VX-1163",
        "VX-1164",
        "VX-1165",
        "VX-1166",
        "VX-1167",
        "VX-1168",
        "VX-1169",
        "VX-1170",
        "VX-1171",
        "VX-1172",
        "VX-1173",
        "VX-1174",
        "VX-1175",
        "VX-1176",
        "VX-1177",
        "VX-1178",
        "VX-1179",
        "VX-1180",
    ] {
        assert!(PLAN.contains(row), "plan must include {row}");
    }
}

#[test]
fn artifact_integrity_archive_coverage_reuses_existing_provenance_attestation_release_gate_and_publication_boundaries() {
    for required in [
        "VX-1161..VX-1180",
        "release_artifact_integrity_index",
        "consumer_artifact_verification_protocol",
        "release_metadata_anti_rollback_policy",
        "source_archive_durability_map",
        "public_release_supply_chain_provenance",
        "reproducible_build_release_attestation",
        "release_workflow_attestation_policy",
        "final_release_gate_manifest",
        "publication_boundary",
        "dedup_seam",
        "proof_gate",
    ] {
        assert!(
            COVERAGE.contains(required),
            "artifact integrity archive tranche coverage must include {required}"
        );
    }
}
