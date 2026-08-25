//! Public intake response governance test suite.

const LEDGER: &str = include_str!("../../../docs/optimization/RESEARCH_SOURCE_LEDGER.toml");
const PLAN: &str = include_str!("../../../docs/optimization/ALL_AXES_ACCELERATION_PLAN.md");
const INTAKE: &str =
    include_str!("../../../docs/optimization/PUBLIC_ISSUE_SUPPORT_INTAKE_POLICY.toml");
const RESPONSE: &str =
    include_str!("../../../docs/optimization/PRIVATE_VULNERABILITY_RESPONSE_RUNBOOK.toml");
const ESCALATION: &str =
    include_str!("../../../docs/optimization/MAINTAINER_OWNERSHIP_ESCALATION_MAP.toml");
const FEEDBACK: &str =
    include_str!("../../../docs/optimization/RELEASE_HEALTH_FEEDBACK_LOOP.toml");
const COVERAGE: &str = include_str!(
    "../../../docs/optimization/END_TO_END_PUBLIC_INTAKE_RESPONSE_TRANCHE_COVERAGE.toml"
);

#[test]
fn public_intake_response_sources_are_registered() {
    for key in [
        "GITHUB_ISSUE_FORMS",
        "GITHUB_SUPPORT_RESOURCES",
        "GITHUB_LABELS",
        "GITHUB_CODEOWNERS",
        "GITHUB_PRIVATE_VULNERABILITY_REPORTING",
        "FIRST_PSIRT_SERVICES_FRAMEWORK",
    ] {
        assert!(LEDGER.contains(key), "research source ledger must include {key}");
    }
}

#[test]
fn public_issue_support_intake_policy_records_forms_support_labels_security_misroutes_and_privacy_boundaries() {
    for required in [
        "intake_id",
        "surface",
        "required_reporter_inputs",
        "default_labels",
        "routing_policy",
        "security_misroute_policy",
        "privacy_boundary",
        "release_gate_link",
        "public-bug-report",
        "performance-regression-report",
        "docs-install-support",
        "public-security-misroute",
    ] {
        assert!(
            INTAKE.contains(required),
            "public issue support intake policy must include {required}"
        );
    }
}

#[test]
fn private_vulnerability_response_runbook_records_triage_advisory_incident_support_and_gate_effects() {
    for required in [
        "response_id",
        "intake_channel",
        "triage_policy",
        "collaboration_policy",
        "affectedness_policy",
        "remediation_policy",
        "support_version_policy",
        "publication_policy",
        "release_gate_effect",
        "private-report-triage",
        "accepted-draft-advisory",
        "active-exploitation-incident",
    ] {
        assert!(
            RESPONSE.contains(required),
            "private vulnerability response runbook must include {required}"
        );
    }
}

#[test]
fn maintainer_ownership_escalation_map_links_labels_codeowners_security_managers_owner_lanes_and_boundaries() {
    for required in [
        "escalation_id",
        "intake_surface",
        "routing_key_policy",
        "owner_authority",
        "codeowners_policy",
        "security_manager_policy",
        "fallback_policy",
        "boundary_policy",
        "public-issue-owner-route",
        "private-security-owner-route",
        "release-health-owner-route",
    ] {
        assert!(
            ESCALATION.contains(required),
            "maintainer ownership escalation map must include {required}"
        );
    }
}

#[test]
fn release_health_feedback_loop_maps_security_docs_perf_and_operator_signals_to_release_gate_effects() {
    for required in [
        "signal_id",
        "input_surface",
        "normalization_policy",
        "evidence_link",
        "owner_route",
        "release_gate_effect",
        "feedback_artifact",
        "privacy_boundary",
        "accepted-security-report",
        "docs-install-regression",
        "performance-regression",
        "operator-diagnostic-cluster",
    ] {
        assert!(
            FEEDBACK.contains(required),
            "release health feedback loop must include {required}"
        );
    }
}

#[test]
fn plan_contains_public_intake_response_rows() {
    for row in [
        "VX-1141",
        "VX-1142",
        "VX-1143",
        "VX-1144",
        "VX-1145",
        "VX-1146",
        "VX-1147",
        "VX-1148",
        "VX-1149",
        "VX-1150",
        "VX-1151",
        "VX-1152",
        "VX-1153",
        "VX-1154",
        "VX-1155",
        "VX-1156",
        "VX-1157",
        "VX-1158",
        "VX-1159",
        "VX-1160",
    ] {
        assert!(PLAN.contains(row), "plan must include {row}");
    }
}

#[test]
fn public_intake_response_coverage_preserves_existing_security_release_and_repository_authorities() {
    for required in [
        "VX-1141..VX-1160",
        "public_issue_support_intake_policy",
        "private_vulnerability_response_runbook",
        "maintainer_ownership_escalation_map",
        "release_health_feedback_loop",
        "repository_security_disclosure_policy",
        "security_advisory_exchange_policy",
        "public_repository_health_governance",
        "publication_boundary",
        "dedup_seam",
        "proof_gate",
    ] {
        assert!(
            COVERAGE.contains(required),
            "public intake response tranche coverage must include {required}"
        );
    }
}
