//! Staged rollout governance test suite.

const LEDGER: &str = include_str!("../../../docs/optimization/RESEARCH_SOURCE_LEDGER.toml");
const PLAN: &str = include_str!("../../../docs/optimization/ALL_AXES_ACCELERATION_PLAN.md");
const ROLLOUT: &str =
    include_str!("../../../docs/optimization/RELEASE_ROLLOUT_STRATEGY_POLICY.toml");
const UPGRADE: &str =
    include_str!("../../../docs/optimization/KUBERNETES_HELM_UPGRADE_POLICY.toml");
const CANARY: &str =
    include_str!("../../../docs/optimization/PROGRESSIVE_DELIVERY_CANARY_POLICY.toml");
const DECISION: &str =
    include_str!("../../../docs/optimization/ROLLBACK_FORWARD_FIX_DECISION_POLICY.toml");
const COVERAGE: &str =
    include_str!("../../../docs/optimization/END_TO_END_STAGED_ROLLOUT_TRANCHE_COVERAGE.toml");

#[test]
fn staged_rollout_sources_are_registered() {
    for key in [
        "KUBERNETES_DEPLOYMENTS",
        "KUBECTL_ROLLOUT",
        "HELM_UPGRADE",
        "HELM_ROLLBACK",
        "ARGO_ROLLOUTS_CANARY",
        "ARGO_ROLLOUTS_BLUEGREEN",
    ] {
        assert!(LEDGER.contains(key), "research source ledger must include {key}");
    }
}

#[test]
fn release_rollout_strategy_records_phases_artifact_identity_promotion_abort_observation_owner_and_publication_boundary() {
    for required in [
        "strategy_id",
        "deployment_surface",
        "artifact_identity_policy",
        "phase_policy",
        "promotion_gate_policy",
        "abort_gate_policy",
        "observation_policy",
        "owner_route",
        "publication_boundary",
        "public-vyre-operator-release",
        "public-vyre-package-release",
    ] {
        assert!(
            ROLLOUT.contains(required),
            "release rollout strategy policy must include {required}"
        );
    }
}

#[test]
fn kubernetes_helm_upgrade_policy_records_rolling_update_progress_deadline_upgrade_rollback_history_waits_and_anti_rollback() {
    for required in [
        "upgrade_id",
        "surface_policy",
        "rolling_update_policy",
        "progress_deadline_policy",
        "helm_upgrade_policy",
        "helm_rollback_policy",
        "history_policy",
        "readiness_wait_policy",
        "anti_rollback_policy",
        "kubernetes-deployment-rollout",
        "helm-chart-upgrade-rollback",
    ] {
        assert!(
            UPGRADE.contains(required),
            "kubernetes helm upgrade policy must include {required}"
        );
    }
}

#[test]
fn progressive_delivery_policy_records_canary_bluegreen_traffic_analysis_promotion_abort_services_scale_and_routes() {
    for required in [
        "delivery_id",
        "strategy",
        "traffic_policy",
        "analysis_policy",
        "promotion_policy",
        "abort_policy",
        "service_policy",
        "scale_policy",
        "owner_route",
        "canary-analysis-gated-release",
        "bluegreen-preview-gated-release",
    ] {
        assert!(
            CANARY.contains(required),
            "progressive delivery canary policy must include {required}"
        );
    }
}

#[test]
fn rollback_forward_fix_decision_policy_records_triggers_inputs_rollback_forward_pause_verification_privacy_and_gate_effects() {
    for required in [
        "decision_id",
        "trigger_policy",
        "decision_inputs",
        "rollback_condition",
        "forward_fix_condition",
        "pause_condition",
        "verification_policy",
        "privacy_boundary",
        "release_gate_effect",
        "verified-rollback-decision",
        "security-forward-fix-or-disablement",
    ] {
        assert!(
            DECISION.contains(required),
            "rollback forward fix decision policy must include {required}"
        );
    }
}

#[test]
fn plan_contains_staged_rollout_rows() {
    for row in [
        "VX-1261",
        "VX-1262",
        "VX-1263",
        "VX-1264",
        "VX-1265",
        "VX-1266",
        "VX-1267",
        "VX-1268",
        "VX-1269",
        "VX-1270",
        "VX-1271",
        "VX-1272",
        "VX-1273",
        "VX-1274",
        "VX-1275",
        "VX-1276",
        "VX-1277",
        "VX-1278",
        "VX-1279",
        "VX-1280",
    ] {
        assert!(PLAN.contains(row), "plan must include {row}");
    }
}

#[test]
fn staged_rollout_coverage_reuses_deployment_artifact_readiness_release_health_publication_and_decision_authorities() {
    for required in [
        "VX-1261..VX-1280",
        "release_rollout_strategy_policy",
        "kubernetes_helm_upgrade_policy",
        "progressive_delivery_canary_policy",
        "rollback_forward_fix_decision_policy",
        "artifact_integrity_archive_coverage",
        "operational_readiness_coverage",
        "operator_deployment_surface_matrix",
        "release_health_feedback_loop",
        "publication_boundary",
        "dedup_seam",
        "proof_gate",
    ] {
        assert!(
            COVERAGE.contains(required),
            "staged rollout tranche coverage must include {required}"
        );
    }
}
