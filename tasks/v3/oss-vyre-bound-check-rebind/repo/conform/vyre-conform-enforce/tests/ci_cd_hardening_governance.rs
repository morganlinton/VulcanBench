//! Ci cd hardening governance test suite.

const LEDGER: &str = include_str!("../../../docs/optimization/RESEARCH_SOURCE_LEDGER.toml");
const PLAN: &str = include_str!("../../../docs/optimization/ALL_AXES_ACCELERATION_PLAN.md");
const PERMISSIONS: &str =
    include_str!("../../../docs/optimization/WORKFLOW_PERMISSION_BOUNDARY.toml");
const PINNING: &str =
    include_str!("../../../docs/optimization/ACTIONS_DEPENDENCY_PINNING_POLICY.toml");
const RUNNER_CACHE: &str =
    include_str!("../../../docs/optimization/CI_RUNNER_CACHE_ARTIFACT_POLICY.toml");
const ATTESTATION: &str =
    include_str!("../../../docs/optimization/RELEASE_WORKFLOW_ATTESTATION_POLICY.toml");
const COVERAGE: &str =
    include_str!("../../../docs/optimization/END_TO_END_CI_CD_HARDENING_TRANCHE_COVERAGE.toml");

#[test]
fn ci_cd_hardening_sources_are_registered() {
    for key in [
        "GITHUB_ACTIONS_SECURE_USE",
        "GITHUB_ACTIONS_TOKEN_PERMISSIONS",
        "GITHUB_ACTIONS_OIDC",
        "GITHUB_ARTIFACT_ATTESTATIONS",
        "GITHUB_ACTIONS_DEPENDENCY_CACHING",
    ] {
        assert!(LEDGER.contains(key), "research source ledger must include {key}");
    }
}

#[test]
fn workflow_permission_boundary_records_trust_token_oidc_secret_input_and_private_boundaries() {
    for required in [
        "workflow_class",
        "trigger_trust_policy",
        "default_permissions_policy",
        "job_permissions_policy",
        "oidc_policy",
        "secret_policy",
        "untrusted_input_policy",
        "private_santh_boundary",
        "public-pr-validation",
        "public-release-publication",
    ] {
        assert!(
            PERMISSIONS.contains(required),
            "workflow permission boundary must include {required}"
        );
    }
}

#[test]
fn actions_dependency_pinning_policy_requires_full_sha_review_updates_and_script_injection_controls() {
    for required in [
        "dependency_class",
        "pinning_policy",
        "source_review_policy",
        "update_policy",
        "script_injection_policy",
        "reusable_workflow_policy",
        "allowlist_authority",
        "publication_gate",
        "third-party-action",
        "pin-to-full-length-commit-sha",
    ] {
        assert!(
            PINNING.contains(required),
            "actions dependency pinning policy must include {required}"
        );
    }
}

#[test]
fn runner_cache_artifact_policy_separates_pr_validation_from_release_evidence() {
    for required in [
        "ci_surface",
        "runner_policy",
        "cache_policy",
        "artifact_policy",
        "log_policy",
        "private_boundary_policy",
        "retention_policy",
        "negative_case",
        "public-pr-validation",
        "public-release-publication",
    ] {
        assert!(
            RUNNER_CACHE.contains(required),
            "runner cache artifact policy must include {required}"
        );
    }
}

#[test]
fn release_workflow_attestation_policy_links_artifact_attestations_slsa_sbom_and_reproducibility() {
    for required in [
        "attestation_surface",
        "subject_policy",
        "predicate_policy",
        "signer_policy",
        "verification_policy",
        "sbom_policy",
        "reproducibility_link",
        "publication_boundary",
        "public-crate-release",
        "benchmark-and-dogfood-evidence",
    ] {
        assert!(
            ATTESTATION.contains(required),
            "release workflow attestation policy must include {required}"
        );
    }
}

#[test]
fn plan_contains_ci_cd_hardening_rows() {
    for row in [
        "VX-1101",
        "VX-1102",
        "VX-1103",
        "VX-1104",
        "VX-1105",
        "VX-1106",
        "VX-1107",
        "VX-1108",
        "VX-1109",
        "VX-1110",
        "VX-1111",
        "VX-1112",
        "VX-1113",
        "VX-1114",
        "VX-1115",
        "VX-1116",
        "VX-1117",
        "VX-1118",
        "VX-1119",
        "VX-1120",
    ] {
        assert!(PLAN.contains(row), "plan must include {row}");
    }
}

#[test]
fn ci_cd_hardening_coverage_preserves_shared_dedup_seams() {
    for required in [
        "VX-1101..VX-1120",
        "workflow_permission_boundary",
        "actions_dependency_pinning_policy",
        "ci_runner_cache_artifact_policy",
        "release_workflow_attestation_policy",
        "final_release_gate_manifest",
        "public_repository_rulesets",
        "publication_boundary",
        "dedup_seam",
        "proof_gate",
    ] {
        assert!(
            COVERAGE.contains(required),
            "CI/CD hardening tranche coverage must include {required}"
        );
    }
}
