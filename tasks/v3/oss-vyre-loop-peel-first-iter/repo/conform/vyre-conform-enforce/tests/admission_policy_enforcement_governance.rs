//! Admission policy enforcement governance test suite.

const LEDGER: &str = include_str!("../../../docs/optimization/RESEARCH_SOURCE_LEDGER.toml");
const PLAN: &str = include_str!("../../../docs/optimization/ALL_AXES_ACCELERATION_PLAN.md");
const ADMISSION: &str =
    include_str!("../../../docs/optimization/KUBERNETES_ADMISSION_POLICY_ENFORCEMENT.toml");
const PARITY: &str =
    include_str!("../../../docs/optimization/POLICY_ENGINE_PARITY_DEPLOYMENT_MATRIX.toml");
const EXCEPTIONS: &str =
    include_str!("../../../docs/optimization/ADMISSION_EXCEPTION_BREAKGLASS_POLICY.toml");
const AUDIT: &str = include_str!("../../../docs/optimization/ADMISSION_AUDIT_EVIDENCE_POLICY.toml");
const COVERAGE: &str =
    include_str!("../../../docs/optimization/END_TO_END_ADMISSION_POLICY_ENFORCEMENT_TRANCHE_COVERAGE.toml");

#[test]
fn admission_policy_enforcement_sources_are_registered() {
    for key in [
        "KUBERNETES_ADMISSION_CONTROLLERS",
        "KUBERNETES_DYNAMIC_ADMISSION",
        "KUBERNETES_VALIDATING_ADMISSION_POLICY",
        "KUBERNETES_AUDITING",
        "OPA_GATEKEEPER",
        "KYVERNO_VALIDATE_RULES",
    ] {
        assert!(LEDGER.contains(key), "research source ledger must include {key}");
    }
}

#[test]
fn kubernetes_admission_policy_records_phases_validation_mutation_failure_match_authority_and_gate_effects() {
    for required in [
        "enforcement_id",
        "target_surface",
        "admission_phase_policy",
        "validation_policy",
        "mutation_policy",
        "failure_policy",
        "match_scope_policy",
        "authority_link_policy",
        "release_gate_effect",
        "public-vyre-workload-admission",
        "autoscaling-and-capacity-admission",
    ] {
        assert!(
            ADMISSION.contains(required),
            "kubernetes admission policy enforcement must include {required}"
        );
    }
}

#[test]
fn policy_engine_parity_matrix_records_decision_modes_parameters_audit_negative_cases_portability_dedup_and_gate_effects() {
    for required in [
        "engine_id",
        "policy_surface",
        "decision_mode_policy",
        "parameter_policy",
        "audit_mode_policy",
        "negative_case_policy",
        "portability_policy",
        "dedup_authority_policy",
        "release_gate_effect",
        "native-validating-admission-policy",
        "gatekeeper-kyverno-runtime-policy",
    ] {
        assert!(
            PARITY.contains(required),
            "policy engine parity deployment matrix must include {required}"
        );
    }
}

#[test]
fn admission_exception_policy_records_triggers_scope_authorization_expiry_audit_remediation_privacy_and_gate_effects() {
    for required in [
        "exception_id",
        "trigger_policy",
        "scope_policy",
        "authorization_policy",
        "expiry_policy",
        "audit_policy",
        "remediation_policy",
        "privacy_boundary",
        "release_gate_effect",
        "operator-recovery-breakglass",
        "policy-audit-to-enforce-promotion",
    ] {
        assert!(
            EXCEPTIONS.contains(required),
            "admission exception breakglass policy must include {required}"
        );
    }
}

#[test]
fn admission_audit_policy_records_capture_correlation_metrics_retention_redaction_release_health_and_operator_results() {
    for required in [
        "audit_id",
        "event_surface",
        "audit_capture_policy",
        "decision_correlation_policy",
        "metric_policy",
        "retention_policy",
        "redaction_policy",
        "release_health_policy",
        "operator_result_policy",
        "admission-denial-audit-evidence",
        "policy-engine-drift-audit-evidence",
    ] {
        assert!(
            AUDIT.contains(required),
            "admission audit evidence policy must include {required}"
        );
    }
}

#[test]
fn plan_contains_admission_policy_enforcement_rows() {
    for row in [
        "VX-1341",
        "VX-1342",
        "VX-1343",
        "VX-1344",
        "VX-1345",
        "VX-1346",
        "VX-1347",
        "VX-1348",
        "VX-1349",
        "VX-1350",
        "VX-1351",
        "VX-1352",
        "VX-1353",
        "VX-1354",
        "VX-1355",
        "VX-1356",
        "VX-1357",
        "VX-1358",
        "VX-1359",
        "VX-1360",
    ] {
        assert!(PLAN.contains(row), "plan must include {row}");
    }
}

#[test]
fn admission_policy_coverage_reuses_plan_policy_deployment_capacity_readiness_publication_and_dedup_authorities() {
    for required in [
        "VX-1341..VX-1360",
        "kubernetes_admission_policy_enforcement",
        "policy_engine_parity_deployment_matrix",
        "admission_exception_breakglass_policy",
        "admission_audit_evidence_policy",
        "plan_policy_enforcement_coverage",
        "operator_deployment_coverage",
        "capacity_autoscaling_coverage",
        "operational_readiness_coverage",
        "publication_boundary",
        "dedup_seam",
        "proof_gate",
    ] {
        assert!(
            COVERAGE.contains(required),
            "admission policy enforcement tranche coverage must include {required}"
        );
    }
}
