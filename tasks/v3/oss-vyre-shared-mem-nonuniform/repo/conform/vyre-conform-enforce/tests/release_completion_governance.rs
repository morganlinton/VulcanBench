//! Release completion governance test suite.

const LEDGER: &str = include_str!("../../../docs/optimization/RESEARCH_SOURCE_LEDGER.toml");
const GATES: &str = include_str!("../../../docs/optimization/FINAL_RELEASE_GATE_MANIFEST.toml");
const REPRO: &str =
    include_str!("../../../docs/optimization/REPRODUCIBLE_BUILD_RELEASE_ATTESTATION.toml");
const SECURITY: &str =
    include_str!("../../../docs/optimization/SECURITY_INSIGHTS_RELEASE_SUMMARY.toml");
const DOGFOOD: &str = include_str!("../../../docs/optimization/DOGFOOD_WORKFLOW_EVIDENCE.toml");
const MATRIX: &str =
    include_str!("../../../docs/optimization/COMPLETION_AUDIT_EVIDENCE_MATRIX.toml");
const COVERAGE: &str =
    include_str!("../../../docs/optimization/FINAL_COMPLETION_GOVERNANCE_TRANCHE_COVERAGE.toml");

#[test]
fn final_completion_primary_sources_are_registered() {
    for key in [
        "CARGO_PUBLISH",
        "REPRODUCIBLE_BUILDS",
        "OPENSSF_SECURITY_INSIGHTS",
        "GITHUB_ACTIONS_WORKFLOW",
    ] {
        assert!(LEDGER.contains(key), "research source ledger must include {key}");
    }
}

#[test]
fn final_release_gate_manifest_records_gate_artifacts_boundaries_and_completion_blockers() {
    for required in [
        "gate_id",
        "gate_class",
        "command_class",
        "required_artifact",
        "publication_boundary",
        "private_santh_policy",
        "public_vyre_policy",
        "completion_blocker_if_missing",
    ] {
        assert!(GATES.contains(required), "final release gate manifest must include {required}");
    }
}

#[test]
fn reproducible_build_attestation_records_source_environment_instructions_outputs_and_provenance() {
    for required in [
        "attestation_id",
        "source_digest",
        "build_environment_digest",
        "build_instructions",
        "output_digest",
        "comparison_policy",
        "provenance_link",
        "publication_class",
    ] {
        assert!(
            REPRO.contains(required),
            "reproducible build release attestation must include {required}"
        );
    }
}

#[test]
fn security_insights_release_summary_records_public_scope_and_private_santh_exclusion() {
    for required in [
        "summary_id",
        "security_contact_policy",
        "vulnerability_reporting_policy",
        "dependency_review_policy",
        "supported_versions_policy",
        "artifact_scope",
        "machine_readable_summary",
        "private-santh-worktree-excluded",
    ] {
        assert!(
            SECURITY.contains(required),
            "security insights release summary must include {required}"
        );
    }
}

#[test]
fn dogfood_workflow_evidence_records_inputs_artifacts_operator_results_and_privacy_gates() {
    for required in [
        "workflow_id",
        "workflow_class",
        "input_scope",
        "expected_artifacts",
        "operator_visible_result",
        "privacy_gate",
        "publication_boundary",
        "completion_blocker_if_missing",
    ] {
        assert!(
            DOGFOOD.contains(required),
            "dogfood workflow evidence must include {required}"
        );
    }
}

#[test]
fn completion_audit_matrix_blocks_completion_without_direct_authoritative_evidence() {
    for required in [
        "requirement_id",
        "scope",
        "authoritative_evidence",
        "evidence_status",
        "completion_decision",
        "blocker_if_unverified",
        "not-complete-without-user-authorized-validation",
        "source-ledger-key-coverage-audit",
    ] {
        assert!(
            MATRIX.contains(required),
            "completion audit evidence matrix must include {required}"
        );
    }
}

#[test]
fn final_completion_tranche_coverage_preserves_shared_dedup_seams() {
    for required in [
        "VX-981..VX-1000",
        "final_release_gate_manifest",
        "reproducible_build_release_attestation",
        "security_insights_release_summary",
        "dogfood_workflow_evidence",
        "completion_audit_evidence_matrix",
        "source_ledger_coverage",
        "publication_boundary",
        "dedup_seam",
        "proof_gate",
    ] {
        assert!(
            COVERAGE.contains(required),
            "final completion governance tranche coverage must include {required}"
        );
    }
}
