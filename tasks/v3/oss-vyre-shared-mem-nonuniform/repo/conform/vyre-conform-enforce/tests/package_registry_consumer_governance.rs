//! Package registry consumer governance test suite.

const LEDGER: &str = include_str!("../../../docs/optimization/RESEARCH_SOURCE_LEDGER.toml");
const PLAN: &str = include_str!("../../../docs/optimization/ALL_AXES_ACCELERATION_PLAN.md");
const REGISTRY: &str =
    include_str!("../../../docs/optimization/CRATES_IO_REGISTRY_PUBLICATION_POLICY.toml");
const DOCS: &str =
    include_str!("../../../docs/optimization/DOCS_RS_RUSTDOC_PUBLICATION_POLICY.toml");
const CONSUMER: &str =
    include_str!("../../../docs/optimization/CONSUMER_INSTALL_COMPATIBILITY_MATRIX.toml");
const COMPAT: &str =
    include_str!("../../../docs/optimization/PUBLIC_API_SEMVER_MSRV_POLICY.toml");
const COVERAGE: &str = include_str!(
    "../../../docs/optimization/END_TO_END_PACKAGE_REGISTRY_CONSUMER_TRANCHE_COVERAGE.toml"
);

#[test]
fn package_registry_consumer_sources_are_registered_and_existing_semver_api_keys_are_reused() {
    for key in [
        "CARGO_PUBLISH",
        "CARGO_RUST_VERSION_FIELD",
        "DOCS_RS_BUILDS",
        "DOCS_RS_METADATA",
        "RUSTDOC_BOOK",
        "SEMVER_2_0",
        "RUST_API_GUIDELINES",
    ] {
        assert!(LEDGER.contains(key), "research source ledger must include {key}");
    }
}

#[test]
fn crates_io_registry_publication_policy_records_permanence_owners_yanking_package_contents_and_boundaries() {
    for required in [
        "registry_surface",
        "crate_name_policy",
        "publish_permanence_policy",
        "owner_policy",
        "yank_policy",
        "package_contents_policy",
        "dry_run_policy",
        "private_boundary_policy",
        "public-vyre-crate",
        "feature-gated-tool-subcrate",
    ] {
        assert!(
            REGISTRY.contains(required),
            "crates.io registry publication policy must include {required}"
        );
    }
}

#[test]
fn docs_rs_rustdoc_publication_policy_records_readme_metadata_features_targets_rustdoc_and_private_boundaries() {
    for required in [
        "docs_surface",
        "readme_policy",
        "metadata_docs_rs_policy",
        "feature_documentation_policy",
        "target_policy",
        "rustdoc_policy",
        "build_script_policy",
        "private_boundary_policy",
        "public-vyre-docs-rs",
        "public-tool-subcrate-docs",
    ] {
        assert!(
            DOCS.contains(required),
            "docs.rs rustdoc publication policy must include {required}"
        );
    }
}

#[test]
fn consumer_install_matrix_covers_cli_library_and_subcrate_consumption_modes() {
    for required in [
        "consumer_mode",
        "entrypoint",
        "cargo_command_policy",
        "feature_policy",
        "msrv_policy",
        "semver_policy",
        "public_docs_policy",
        "negative_case",
        "cargo-install-cli",
        "cargo-add-library",
        "cargo-add-tool-subcrate",
    ] {
        assert!(
            CONSUMER.contains(required),
            "consumer install compatibility matrix must include {required}"
        );
    }
}

#[test]
fn public_api_semver_msrv_policy_links_api_snapshots_migrations_docs_consumers_and_publication_gates() {
    for required in [
        "compat_surface",
        "api_snapshot_policy",
        "semver_change_policy",
        "msrv_change_policy",
        "migration_policy",
        "docs_gate_policy",
        "consumer_mode_link",
        "publication_gate",
        "public-library-api",
        "cli-and-config-surface",
        "feature-gated-tool-subcrate",
    ] {
        assert!(
            COMPAT.contains(required),
            "public API semver MSRV policy must include {required}"
        );
    }
}

#[test]
fn plan_contains_package_registry_consumer_rows() {
    for row in [
        "VX-1121",
        "VX-1122",
        "VX-1123",
        "VX-1124",
        "VX-1125",
        "VX-1126",
        "VX-1127",
        "VX-1128",
        "VX-1129",
        "VX-1130",
        "VX-1131",
        "VX-1132",
        "VX-1133",
        "VX-1134",
        "VX-1135",
        "VX-1136",
        "VX-1137",
        "VX-1138",
        "VX-1139",
        "VX-1140",
    ] {
        assert!(PLAN.contains(row), "plan must include {row}");
    }
}

#[test]
fn package_registry_consumer_coverage_preserves_shared_dedup_seams() {
    for required in [
        "VX-1121..VX-1140",
        "crates_io_registry_publication_policy",
        "docs_rs_rustdoc_publication_policy",
        "consumer_install_compatibility_matrix",
        "public_api_semver_msrv_policy",
        "package_metadata_publication_policy",
        "crate_boundary_feature_matrix",
        "publication_boundary",
        "dedup_seam",
        "proof_gate",
    ] {
        assert!(
            COVERAGE.contains(required),
            "package registry consumer tranche coverage must include {required}"
        );
    }
}
