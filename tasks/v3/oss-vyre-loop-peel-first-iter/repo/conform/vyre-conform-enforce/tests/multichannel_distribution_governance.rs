//! Multichannel distribution governance test suite.

const LEDGER: &str = include_str!("../../../docs/optimization/RESEARCH_SOURCE_LEDGER.toml");
const PLAN: &str = include_str!("../../../docs/optimization/ALL_AXES_ACCELERATION_PLAN.md");
const SURFACES: &str =
    include_str!("../../../docs/optimization/MULTICHANNEL_DISTRIBUTION_SURFACE_MATRIX.toml");
const OCI: &str =
    include_str!("../../../docs/optimization/OCI_CONTAINER_IMAGE_PUBLICATION_POLICY.toml");
const HOMEBREW: &str =
    include_str!("../../../docs/optimization/HOMEBREW_TAP_BOTTLE_PUBLICATION_POLICY.toml");
const BINARIES: &str =
    include_str!("../../../docs/optimization/BINARY_ASSET_PLATFORM_MATRIX.toml");
const COVERAGE: &str = include_str!(
    "../../../docs/optimization/END_TO_END_MULTICHANNEL_DISTRIBUTION_TRANCHE_COVERAGE.toml"
);

#[test]
fn multichannel_distribution_sources_are_registered() {
    for key in [
        "OCI_IMAGE_SPEC",
        "OCI_DISTRIBUTION_SPEC",
        "GITHUB_CONTAINER_REGISTRY",
        "HOMEBREW_FORMULA_COOKBOOK",
        "HOMEBREW_BOTTLES",
        "HOMEBREW_SHA256_POLICY",
    ] {
        assert!(LEDGER.contains(key), "research source ledger must include {key}");
    }
}

#[test]
fn multichannel_surface_matrix_keeps_channels_install_commands_verification_release_authority_and_private_boundaries_distinct() {
    for required in [
        "channel_id",
        "distribution_surface",
        "public_endpoint_policy",
        "artifact_identity_policy",
        "install_command_policy",
        "verification_policy",
        "release_authority",
        "private_boundary_policy",
        "canonical-cargo-crate",
        "github-release-binary-assets",
        "ghcr-oci-image",
        "homebrew-formula-and-bottle",
    ] {
        assert!(
            SURFACES.contains(required),
            "multi-channel distribution surface matrix must include {required}"
        );
    }
}

#[test]
fn oci_container_policy_records_registry_manifest_platform_layers_annotations_tags_digest_pulls_sboms_and_boundaries() {
    for required in [
        "image_id",
        "registry_policy",
        "manifest_policy",
        "platform_index_policy",
        "layer_policy",
        "annotation_policy",
        "tag_policy",
        "digest_pull_policy",
        "sbom_attestation_policy",
        "private_boundary_policy",
        "vyre-cli-oci-image",
        "vyre-dogfood-oci-image",
    ] {
        assert!(
            OCI.contains(required),
            "OCI container image publication policy must include {required}"
        );
    }
}

#[test]
fn homebrew_policy_records_formula_source_license_dependencies_tests_bottles_sha256_tap_boundaries() {
    for required in [
        "formula_id",
        "formula_policy",
        "source_url_policy",
        "license_policy",
        "dependency_policy",
        "test_policy",
        "bottle_policy",
        "checksum_policy",
        "tap_boundary_policy",
        "vyre-homebrew-formula",
        "vyre-homebrew-bottle",
    ] {
        assert!(
            HOMEBREW.contains(required),
            "Homebrew tap bottle publication policy must include {required}"
        );
    }
}

#[test]
fn binary_asset_matrix_records_platform_archives_entrypoints_ancillary_assets_checksums_signatures_docs_parity_and_boundaries() {
    for required in [
        "asset_id",
        "platform_triple",
        "archive_format",
        "binary_entrypoint",
        "ancillary_assets_policy",
        "checksum_policy",
        "signature_policy",
        "install_doc_policy",
        "channel_parity_policy",
        "private_boundary_policy",
        "vyre-linux-x86_64-gnu",
        "vyre-linux-aarch64-gnu",
        "vyre-macos-universal",
        "vyre-windows-x86_64-msvc",
    ] {
        assert!(
            BINARIES.contains(required),
            "binary asset platform matrix must include {required}"
        );
    }
}

#[test]
fn plan_contains_multichannel_distribution_rows() {
    for row in [
        "VX-1181",
        "VX-1182",
        "VX-1183",
        "VX-1184",
        "VX-1185",
        "VX-1186",
        "VX-1187",
        "VX-1188",
        "VX-1189",
        "VX-1190",
        "VX-1191",
        "VX-1192",
        "VX-1193",
        "VX-1194",
        "VX-1195",
        "VX-1196",
        "VX-1197",
        "VX-1198",
        "VX-1199",
        "VX-1200",
    ] {
        assert!(PLAN.contains(row), "plan must include {row}");
    }
}

#[test]
fn multichannel_distribution_coverage_reuses_cargo_integrity_verification_final_gate_and_publication_boundaries() {
    for required in [
        "VX-1181..VX-1200",
        "multichannel_distribution_surface_matrix",
        "oci_container_image_publication_policy",
        "homebrew_tap_bottle_publication_policy",
        "binary_asset_platform_matrix",
        "consumer_install_compatibility_matrix",
        "release_artifact_integrity_index",
        "consumer_artifact_verification_protocol",
        "final_release_gate_manifest",
        "publication_boundary",
        "dedup_seam",
        "proof_gate",
    ] {
        assert!(
            COVERAGE.contains(required),
            "multi-channel distribution tranche coverage must include {required}"
        );
    }
}
