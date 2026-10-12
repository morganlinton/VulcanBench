//! Operator deployment governance test suite.

const LEDGER: &str = include_str!("../../../docs/optimization/RESEARCH_SOURCE_LEDGER.toml");
const PLAN: &str = include_str!("../../../docs/optimization/ALL_AXES_ACCELERATION_PLAN.md");
const SURFACES: &str =
    include_str!("../../../docs/optimization/OPERATOR_DEPLOYMENT_SURFACE_MATRIX.toml");
const K8S: &str =
    include_str!("../../../docs/optimization/KUBERNETES_HELM_DEPLOYMENT_POLICY.toml");
const SYSTEMD: &str =
    include_str!("../../../docs/optimization/SYSTEMD_SERVICE_HARDENING_POLICY.toml");
const CONFIG: &str =
    include_str!("../../../docs/optimization/OPERATOR_RUNTIME_CONFIGURATION_POLICY.toml");
const COVERAGE: &str =
    include_str!("../../../docs/optimization/END_TO_END_OPERATOR_DEPLOYMENT_TRANCHE_COVERAGE.toml");

#[test]
fn operator_deployment_sources_are_registered() {
    for key in [
        "KUBERNETES_POD_SECURITY_STANDARDS",
        "KUBERNETES_SECURITY_CONTEXT",
        "KUBERNETES_POD_SECURITY_ADMISSION",
        "KUBERNETES_CONFIGMAP",
        "KUBERNETES_SECRETS",
        "HELM_CHART_BEST_PRACTICES",
        "HELM_CHART_FORMAT",
        "SYSTEMD_SERVICE",
        "SYSTEMD_EXEC",
        "SYSTEMD_RESOURCE_CONTROL",
    ] {
        assert!(LEDGER.contains(key), "research source ledger must include {key}");
    }
}

#[test]
fn operator_deployment_surface_matrix_keeps_runtime_targets_artifact_sources_config_identity_resources_security_and_telemetry_distinct() {
    for required in [
        "deployment_id",
        "deployment_surface",
        "artifact_source_policy",
        "configuration_policy",
        "identity_policy",
        "resource_policy",
        "security_policy",
        "telemetry_policy",
        "publication_boundary",
        "helm-chart-public-operator",
        "raw-kubernetes-manifests",
        "systemd-service-unit",
        "container-run-local",
    ] {
        assert!(
            SURFACES.contains(required),
            "operator deployment surface matrix must include {required}"
        );
    }
}

#[test]
fn kubernetes_helm_policy_records_chart_image_pod_security_context_config_secret_resource_and_probe_controls() {
    for required in [
        "policy_id",
        "chart_policy",
        "image_policy",
        "pod_security_policy",
        "security_context_policy",
        "configuration_policy",
        "secret_policy",
        "resource_policy",
        "probe_policy",
        "public-vyre-helm-chart",
        "public-vyre-kubernetes-manifest",
    ] {
        assert!(
            K8S.contains(required),
            "Kubernetes Helm deployment policy must include {required}"
        );
    }
}

#[test]
fn systemd_service_policy_records_service_exec_identity_filesystem_capability_network_resource_and_secret_hardening() {
    for required in [
        "unit_id",
        "service_policy",
        "exec_policy",
        "identity_policy",
        "filesystem_policy",
        "capability_policy",
        "network_policy",
        "resource_policy",
        "secret_policy",
        "vyre-daemon-systemd-service",
        "vyre-batch-systemd-service",
    ] {
        assert!(
            SYSTEMD.contains(required),
            "systemd service hardening policy must include {required}"
        );
    }
}

#[test]
fn operator_runtime_config_policy_records_allowed_config_secret_resource_telemetry_gpu_validation_and_publication_boundaries() {
    for required in [
        "config_id",
        "surface",
        "allowed_config_policy",
        "secret_boundary_policy",
        "resource_knob_policy",
        "telemetry_policy",
        "gpu_policy",
        "validation_policy",
        "publication_boundary",
        "kubernetes-runtime-config",
        "systemd-runtime-config",
    ] {
        assert!(
            CONFIG.contains(required),
            "operator runtime configuration policy must include {required}"
        );
    }
}

#[test]
fn plan_contains_operator_deployment_rows() {
    for row in [
        "VX-1221",
        "VX-1222",
        "VX-1223",
        "VX-1224",
        "VX-1225",
        "VX-1226",
        "VX-1227",
        "VX-1228",
        "VX-1229",
        "VX-1230",
        "VX-1231",
        "VX-1232",
        "VX-1233",
        "VX-1234",
        "VX-1235",
        "VX-1236",
        "VX-1237",
        "VX-1238",
        "VX-1239",
        "VX-1240",
    ] {
        assert!(PLAN.contains(row), "plan must include {row}");
    }
}

#[test]
fn operator_deployment_coverage_reuses_oci_resource_secret_network_telemetry_and_publication_authorities() {
    for required in [
        "VX-1221..VX-1240",
        "operator_deployment_surface_matrix",
        "kubernetes_helm_deployment_policy",
        "systemd_service_hardening_policy",
        "operator_runtime_configuration_policy",
        "oci_container_image_publication_policy",
        "resource_dos_governance",
        "secret_material_handling_policy",
        "network_security_governance",
        "operator_evidence_governance",
        "publication_boundary",
        "dedup_seam",
        "proof_gate",
    ] {
        assert!(
            COVERAGE.contains(required),
            "operator deployment tranche coverage must include {required}"
        );
    }
}
