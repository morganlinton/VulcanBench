//! Plan boundary governance test suite.

const PANIC_REGISTRY: &str =
    include_str!("../../../docs/optimization/PRODUCTION_PANIC_FALLBACK_REGISTRY.toml");
const API_SNAPSHOTS: &str =
    include_str!("../../../docs/optimization/PUBLIC_API_OWNERSHIP_SNAPSHOTS.toml");
const ARTIFACTS: &str =
    include_str!("../../../docs/optimization/RELEASE_ARTIFACT_PROVENANCE.toml");
const SOURCE_IMPACT: &str =
    include_str!("../../../docs/optimization/SOURCE_IMPACT_GRAPH.toml");
const HELPER_INTAKE: &str =
    include_str!("../../../docs/optimization/HELPER_INTAKE_RECORDS.toml");
const REPO_BOUNDARY: &str =
    include_str!("../../../docs/optimization/REPO_BOUNDARY_PUBLICATION_CHECKS.toml");
const TRANCHE: &str =
    include_str!("../../../docs/optimization/ACTIVE_PLAN_TRANCHE_COVERAGE.toml");

#[test]
fn hygiene_registry_classifies_release_path_panics_and_fallbacks() {
    for required in [
        "test_assertion",
        "documentation_example",
        "compatibility_shim",
        "release_path",
        "release_allowed",
        "VYRE_RELEASE_PATH_EXPECT_UNREGISTERED",
    ] {
        assert!(
            PANIC_REGISTRY.contains(required),
            "panic/fallback registry must include {required}"
        );
    }
}

#[test]
fn public_api_snapshots_require_ownership_and_migration_contracts() {
    for required in [
        "symbol",
        "owner_lane",
        "stability_class",
        "feature_gate",
        "dependency_direction",
        "migration_path",
        "release_proof_id",
    ] {
        assert!(
            API_SNAPSHOTS.contains(required),
            "public API ownership snapshot must include {required}"
        );
    }
}

#[test]
fn release_artifact_provenance_names_generator_and_semantic_validator() {
    for required in [
        "generator_command",
        "command_mode",
        "required_status",
        "semantic_validator",
        "input_digest",
        "output_digest",
        "blocker_list",
    ] {
        assert!(
            ARTIFACTS.contains(required),
            "release artifact provenance must include {required}"
        );
    }
}

#[test]
fn coordination_registries_require_source_impact_helper_intake_and_boundary_checks() {
    for required in [
        "vx_id",
        "touched_path",
        "owner_lane",
        "proof_gate",
        "benchmark_target",
        "superseded_rows",
    ] {
        assert!(
            SOURCE_IMPACT.contains(required),
            "source-impact graph must include {required}"
        );
    }

    for required in [
        "searched_names",
        "operation_phrase",
        "reused_primitive",
        "moved_shared_crate",
        "new_helper_rationale",
        "approval_record",
    ] {
        assert!(
            HELPER_INTAKE.contains(required),
            "helper-intake record must include {required}"
        );
    }

    for required in [
        "url_class",
        "artifact_path_class",
        "package_metadata_class",
        "readme_link_class",
        "release_evidence_class",
        "publication_allowed",
        "VYRE_PUBLICATION_PRIVATE_BOUNDARY_REFUSED",
    ] {
        assert!(
            REPO_BOUNDARY.contains(required),
            "repo-boundary publication check must include {required}"
        );
    }
}

#[test]
fn active_plan_tranche_coverage_requires_evidence_sources_gates_and_dedup_seams() {
    for required in [
        "VX-621..VX-700",
        "construct_tier_coverage",
        "dialect_lattice_coverage",
        "accelerator_route_coverage",
        "source_impact_coverage",
        "public_private_boundary_coverage",
        "local_evidence_required = true",
        "known_source_key_required = true",
        "concrete_work_required = true",
        "proof_gate_required = true",
        "dedup_seam_required = true",
    ] {
        assert!(
            TRANCHE.contains(required),
            "active plan tranche coverage must include {required}"
        );
    }
}
