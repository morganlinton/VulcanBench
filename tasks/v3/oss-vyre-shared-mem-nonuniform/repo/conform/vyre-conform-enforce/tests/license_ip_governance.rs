//! License ip governance test suite.

const LEDGER: &str = include_str!("../../../docs/optimization/RESEARCH_SOURCE_LEDGER.toml");
const PLAN: &str = include_str!("../../../docs/optimization/ALL_AXES_ACCELERATION_PLAN.md");
const LICENSES: &str =
    include_str!("../../../docs/optimization/PUBLIC_RELEASE_LICENSE_MANIFEST.toml");
const NOTICES: &str =
    include_str!("../../../docs/optimization/THIRD_PARTY_NOTICE_ATTRIBUTION.toml");
const PROCESS: &str = include_str!("../../../docs/optimization/LICENSE_COMPLIANCE_PROCESS_MAP.toml");
const METADATA: &str =
    include_str!("../../../docs/optimization/PACKAGE_METADATA_PUBLICATION_POLICY.toml");
const COVERAGE: &str =
    include_str!("../../../docs/optimization/END_TO_END_LICENSE_IP_TRANCHE_COVERAGE.toml");

#[test]
fn license_ip_sources_are_registered() {
    for key in [
        "SPDX_LICENSE_LIST",
        "REUSE_SPEC",
        "OPENCHAIN_ISO_5230",
        "CARGO_MANIFEST_LICENSE_FIELDS",
        "OSI_OPEN_SOURCE_DEFINITION",
    ] {
        assert!(LEDGER.contains(key), "research source ledger must include {key}");
    }
}

#[test]
fn public_license_manifest_records_cargo_spdx_license_files_and_private_boundary() {
    for required in [
        "crate_id",
        "package_scope",
        "license_expression_policy",
        "license_file_policy",
        "spdx_identifier_policy",
        "license_exception_policy",
        "public_package_metadata",
        "private_santh_boundary",
        "vyre-public-root",
        "vyre-tool-subcrate",
    ] {
        assert!(LICENSES.contains(required), "public license manifest must include {required}");
    }
}

#[test]
fn third_party_notices_record_component_source_license_attribution_reuse_and_publication() {
    for required in [
        "component_id",
        "component_source",
        "license_ids",
        "copyright_record",
        "notice_requirement",
        "attribution_text_policy",
        "bundled_artifact_policy",
        "reuse_metadata_policy",
        "publication_boundary",
        "private-local-evidence",
    ] {
        assert!(
            NOTICES.contains(required),
            "third-party notice attribution must include {required}"
        );
    }
}

#[test]
fn license_compliance_process_map_records_roles_triggers_artifacts_archives_and_blockers() {
    for required in [
        "process_id",
        "scope",
        "responsible_seam",
        "review_trigger",
        "required_artifacts",
        "external_inquiry_policy",
        "archive_policy",
        "publication_blocker",
        "public-vyre-license-review",
        "private-santh-exclusion-review",
    ] {
        assert!(
            PROCESS.contains(required),
            "license compliance process map must include {required}"
        );
    }
}

#[test]
fn package_metadata_policy_blocks_private_santh_paths_from_public_crate_metadata() {
    for required in [
        "metadata_id",
        "manifest_fields",
        "include_exclude_policy",
        "readme_policy",
        "repository_policy",
        "documentation_policy",
        "license_boundary",
        "private_boundary",
        "public-crate-metadata",
        "tool-subcrate-metadata",
    ] {
        assert!(
            METADATA.contains(required),
            "package metadata publication policy must include {required}"
        );
    }
}

#[test]
fn plan_contains_license_ip_rows() {
    for row in [
        "VX-1061",
        "VX-1062",
        "VX-1063",
        "VX-1064",
        "VX-1065",
        "VX-1066",
        "VX-1067",
        "VX-1068",
        "VX-1069",
        "VX-1070",
        "VX-1071",
        "VX-1072",
        "VX-1073",
        "VX-1074",
        "VX-1075",
        "VX-1076",
        "VX-1077",
        "VX-1078",
        "VX-1079",
        "VX-1080",
    ] {
        assert!(PLAN.contains(row), "plan must include {row}");
    }
}

#[test]
fn license_ip_coverage_preserves_shared_dedup_seams() {
    for required in [
        "VX-1061..VX-1080",
        "public_release_license_manifest",
        "third_party_notice_attribution",
        "license_compliance_process_map",
        "package_metadata_publication_policy",
        "supply_chain_provenance",
        "publication_boundary",
        "dedup_seam",
        "proof_gate",
    ] {
        assert!(
            COVERAGE.contains(required),
            "license and IP tranche coverage must include {required}"
        );
    }
}
