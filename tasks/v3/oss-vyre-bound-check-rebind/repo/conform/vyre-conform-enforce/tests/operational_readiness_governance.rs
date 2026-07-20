//! Operational readiness governance test suite.

const LEDGER: &str = include_str!("../../../docs/optimization/RESEARCH_SOURCE_LEDGER.toml");
const PLAN: &str = include_str!("../../../docs/optimization/ALL_AXES_ACCELERATION_PLAN.md");
const SLO: &str =
    include_str!("../../../docs/optimization/SERVICE_LEVEL_OBJECTIVE_POLICY.toml");
const ALERTS: &str =
    include_str!("../../../docs/optimization/ALERT_ROUTING_ESCALATION_POLICY.toml");
const RUNBOOKS: &str =
    include_str!("../../../docs/optimization/OPERATOR_RUNBOOK_ROLLBACK_POLICY.toml");
const INCIDENTS: &str =
    include_str!("../../../docs/optimization/INCIDENT_RETROSPECTIVE_EVIDENCE_POLICY.toml");
const COVERAGE: &str =
    include_str!("../../../docs/optimization/END_TO_END_OPERATIONAL_READINESS_TRANCHE_COVERAGE.toml");

#[test]
fn operational_readiness_sources_are_registered() {
    for key in [
        "OPENSLO_SPEC",
        "GOOGLE_SRE_SLO",
        "PROMETHEUS_ALERTING_RULES",
        "PROMETHEUS_RECORDING_RULES",
        "PROMETHEUS_ALERTMANAGER",
        "OPENTELEMETRY_LOGS",
    ] {
        assert!(LEDGER.contains(key), "research source ledger must include {key}");
    }
}

#[test]
fn service_level_objective_policy_records_sli_objective_windows_error_budgets_recording_rules_and_gate_effects() {
    for required in [
        "slo_id",
        "service_surface",
        "sli_query_policy",
        "objective_policy",
        "window_policy",
        "error_budget_policy",
        "recording_rule_policy",
        "release_gate_effect",
        "operator-api-availability",
        "scan-latency-slo",
        "diagnostic-actionability-slo",
    ] {
        assert!(
            SLO.contains(required),
            "service level objective policy must include {required}"
        );
    }
}

#[test]
fn alert_routing_policy_records_expr_duration_labels_annotations_routes_dedup_inhibition_and_runbook_links() {
    for required in [
        "alert_id",
        "signal_source",
        "expr_policy",
        "for_duration_policy",
        "label_policy",
        "annotation_policy",
        "routing_policy",
        "dedup_inhibition_policy",
        "runbook_policy",
        "slo-error-budget-burn",
        "operator-diagnostic-cluster-alert",
        "deployment-saturation-alert",
    ] {
        assert!(
            ALERTS.contains(required),
            "alert routing escalation policy must include {required}"
        );
    }
}

#[test]
fn runbook_rollback_policy_records_triggers_actions_rollback_verification_evidence_privacy_owner_routes_and_gate_effects() {
    for required in [
        "runbook_id",
        "trigger_policy",
        "operator_action_policy",
        "rollback_policy",
        "verification_policy",
        "evidence_capture_policy",
        "privacy_boundary",
        "owner_route",
        "release_gate_effect",
        "rollback-bad-release",
        "resource-saturation-mitigation",
        "security-incident-escalation",
    ] {
        assert!(
            RUNBOOKS.contains(required),
            "operator runbook rollback policy must include {required}"
        );
    }
}

#[test]
fn incident_retrospective_policy_records_timelines_slo_impact_alert_correlation_runbook_actions_remediation_publication_and_feedback() {
    for required in [
        "incident_id",
        "incident_class",
        "timeline_policy",
        "slo_impact_policy",
        "alert_correlation_policy",
        "runbook_action_policy",
        "remediation_link_policy",
        "publication_policy",
        "feedback_loop_policy",
        "operator-reliability-incident",
        "security-incident-retrospective",
    ] {
        assert!(
            INCIDENTS.contains(required),
            "incident retrospective evidence policy must include {required}"
        );
    }
}

#[test]
fn plan_contains_operational_readiness_rows() {
    for row in [
        "VX-1241",
        "VX-1242",
        "VX-1243",
        "VX-1244",
        "VX-1245",
        "VX-1246",
        "VX-1247",
        "VX-1248",
        "VX-1249",
        "VX-1250",
        "VX-1251",
        "VX-1252",
        "VX-1253",
        "VX-1254",
        "VX-1255",
        "VX-1256",
        "VX-1257",
        "VX-1258",
        "VX-1259",
        "VX-1260",
    ] {
        assert!(PLAN.contains(row), "plan must include {row}");
    }
}

#[test]
fn operational_readiness_coverage_reuses_telemetry_release_health_deployment_intake_and_publication_authorities() {
    for required in [
        "VX-1241..VX-1260",
        "service_level_objective_policy",
        "alert_routing_escalation_policy",
        "operator_runbook_rollback_policy",
        "incident_retrospective_evidence_policy",
        "operator_evidence_governance",
        "release_health_feedback_loop",
        "operator_deployment_surface_matrix",
        "public_intake_response_coverage",
        "publication_boundary",
        "dedup_seam",
        "proof_gate",
    ] {
        assert!(
            COVERAGE.contains(required),
            "operational readiness tranche coverage must include {required}"
        );
    }
}
