//! Plan execution governance test suite.

const LEDGER: &str = include_str!("../../../docs/optimization/RESEARCH_SOURCE_LEDGER.toml");
const PLAN: &str = include_str!("../../../docs/optimization/ALL_AXES_ACCELERATION_PLAN.md");
const DAG: &str = include_str!("../../../docs/optimization/PLAN_EXECUTION_DAG.toml");
const AUTHORITY: &str = include_str!("../../../docs/optimization/PLAN_ARTIFACT_AUTHORITY_MAP.toml");
const ATTESTATION: &str =
    include_str!("../../../docs/optimization/PLAN_COMPLETION_ATTESTATION_BUNDLE.toml");
const COVERAGE: &str =
    include_str!("../../../docs/optimization/END_TO_END_EXECUTION_GOVERNANCE_TRANCHE_COVERAGE.toml");

#[test]
fn execution_governance_sources_are_registered() {
    for key in [
        "W3C_PROV_DM",
        "IN_TOTO_ATTESTATION",
        "NIST_SSDF",
        "NIST_OSCAL",
        "OPENSSF_GUAC",
    ] {
        assert!(LEDGER.contains(key), "research source ledger must include {key}");
    }
}

#[test]
fn execution_dag_records_dependencies_authority_gates_and_publication() {
    for required in [
        "node_id",
        "vx_rows",
        "phase",
        "predecessors",
        "owned_artifacts",
        "authority_seam",
        "validation_gate",
        "publication_class",
        "unblock_condition",
        "plan-execution-dependency-graph",
        "artifact-authority-and-dedup-map",
        "completion-attestation-bundle",
    ] {
        assert!(DAG.contains(required), "execution DAG must include {required}");
    }
}

#[test]
fn artifact_authority_map_preserves_one_truth_source_per_registry() {
    for required in [
        "artifact_id",
        "authoritative_file",
        "owning_seam",
        "consumed_by",
        "duplicate_policy",
        "import_rule",
        "publication_boundary",
        "stale_invalidation_policy",
        "no-parallel-source-ledgers",
        "no-second-plan-execution-graph",
        "reference-node-id-not-copy-predecessor-list",
    ] {
        assert!(
            AUTHORITY.contains(required),
            "artifact authority map must include {required}"
        );
    }
}

#[test]
fn completion_attestation_bundle_maps_prov_intoto_ssdf_oscal_and_guac() {
    for required in [
        "attestation_id",
        "predicate_family",
        "prov_entity",
        "prov_activity",
        "prov_agent",
        "materials",
        "products",
        "byproducts",
        "ssdf_practice",
        "oscal_control_family",
        "guac_relationship",
        "publication_class",
        "completion_effect",
        "blocks-completion-until-direct-gate-evidence-exists",
    ] {
        assert!(
            ATTESTATION.contains(required),
            "completion attestation bundle must include {required}"
        );
    }
}

#[test]
fn execution_governance_keeps_completion_blocked_until_direct_evidence_exists() {
    for required in [
        "stale-artifacts-invalidated-and-direct-evidence-recorded-before-completion-claim",
        "blocks-completion-until-direct-gate-evidence-exists",
        "completion_effect",
        "unblock_condition",
    ] {
        assert!(
            DAG.contains(required) || ATTESTATION.contains(required),
            "execution governance must include {required}"
        );
    }
}

#[test]
fn plan_contains_end_to_end_execution_governance_rows() {
    for row in [
        "VX-1001",
        "VX-1002",
        "VX-1003",
        "VX-1004",
        "VX-1005",
        "VX-1006",
        "VX-1007",
        "VX-1008",
        "VX-1009",
        "VX-1010",
        "VX-1011",
        "VX-1012",
        "VX-1013",
        "VX-1014",
        "VX-1015",
        "VX-1016",
        "VX-1017",
        "VX-1018",
        "VX-1019",
        "VX-1020",
    ] {
        assert!(PLAN.contains(row), "plan must include {row}");
    }
}

#[test]
fn end_to_end_execution_governance_coverage_preserves_dedup_seams() {
    for required in [
        "VX-1001..VX-1020",
        "plan_execution_dag",
        "plan_artifact_authority_map",
        "plan_completion_attestation_bundle",
        "source_ledger_coverage",
        "publication_boundary",
        "dedup_seam",
        "proof_gate",
    ] {
        assert!(
            COVERAGE.contains(required),
            "execution governance tranche coverage must include {required}"
        );
    }
}
