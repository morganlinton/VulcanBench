//! Config api governance test suite.

const LEDGER: &str = include_str!("../../../docs/optimization/RESEARCH_SOURCE_LEDGER.toml");
const SCHEMA_GOVERNANCE: &str =
    include_str!("../../../docs/optimization/CONFIG_SCHEMA_GOVERNANCE.toml");
const PRECEDENCE: &str =
    include_str!("../../../docs/optimization/CONFIG_PRECEDENCE_AND_TIERING.toml");
const API: &str = include_str!("../../../docs/optimization/API_COMPATIBILITY_GOVERNANCE.toml");
const CRATES: &str =
    include_str!("../../../docs/optimization/CRATE_BOUNDARY_FEATURE_MATRIX.toml");
const COVERAGE: &str =
    include_str!("../../../docs/optimization/CONFIG_API_GOVERNANCE_TRANCHE_COVERAGE.toml");

#[test]
fn config_api_primary_sources_are_registered() {
    for key in [
        "TOML_1_0",
        "JSON_SCHEMA_2020_12",
        "SEMVER_2_0",
        "RUST_API_GUIDELINES",
    ] {
        assert!(LEDGER.contains(key), "research source ledger must include {key}");
    }
}

#[test]
fn config_schema_governance_declares_registry_formats_schema_contracts_and_diagnostics() {
    for required in [
        "registry_id",
        "file_glob",
        "format",
        "schema_contract",
        "owner_lane",
        "validation_scope",
        "diagnostic_prefix",
        "publication_class",
        "VYRE_CONFIG_SCHEMA_UNKNOWN_FIELD",
        "VYRE_CONFIG_SCHEMA_WRONG_TIER",
    ] {
        assert!(
            SCHEMA_GOVERNANCE.contains(required),
            "config schema governance must include {required}"
        );
    }
}

#[test]
fn config_precedence_and_tiering_preserves_tier_a_cli_and_tier_b_data_boundaries() {
    for required in [
        "compiled_default",
        "tool_toml",
        "cli_override",
        "rules_toml",
        "config_id",
        "tier",
        "operator_visible_effect",
        "precedence_rule",
        "cli_allowed",
        "wiring_gate",
        "VYRE_CONFIG_TIER_B_CLI_OVERRIDE_REFUSED",
    ] {
        assert!(
            PRECEDENCE.contains(required),
            "config precedence and tiering must include {required}"
        );
    }
}

#[test]
fn api_compatibility_governance_records_semver_impact_migrations_and_docs_gates() {
    for required in [
        "api_id",
        "public_surface",
        "stability_class",
        "semver_impact",
        "migration_record",
        "feature_gate",
        "documentation_gate",
        "compatibility_diagnostic",
    ] {
        assert!(
            API.contains(required),
            "API compatibility governance must include {required}"
        );
    }
}

#[test]
fn crate_boundary_feature_matrix_records_consumption_modes_and_dependency_direction() {
    for required in [
        "crate_id",
        "consumption_modes",
        "parent_tool",
        "feature_gates",
        "public_reexports",
        "dependency_direction",
        "thin_wrapper_risk",
        "boundary_diagnostic",
        "VYRE_CRATE_BOUNDARY_THIN_WRAPPER_REFUSED",
    ] {
        assert!(
            CRATES.contains(required),
            "crate boundary feature matrix must include {required}"
        );
    }
}

#[test]
fn config_api_governance_tranche_coverage_preserves_shared_dedup_seams() {
    for required in [
        "VX-801..VX-820",
        "config_schema_governance",
        "config_precedence_tiering",
        "api_compatibility_governance",
        "crate_boundary_feature_matrix",
        "source_ledger_coverage",
        "publication_boundary",
        "dedup_seam",
        "proof_gate",
    ] {
        assert!(
            COVERAGE.contains(required),
            "config/API governance tranche coverage must include {required}"
        );
    }
}
