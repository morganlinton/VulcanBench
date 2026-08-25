//! Operator state recovery governance test suite.

const LEDGER: &str = include_str!("../../../docs/optimization/RESEARCH_SOURCE_LEDGER.toml");
const PLAN: &str = include_str!("../../../docs/optimization/ALL_AXES_ACCELERATION_PLAN.md");
const BACKUP: &str =
    include_str!("../../../docs/optimization/OPERATOR_STATE_BACKUP_RESTORE_POLICY.toml");
const MIGRATION: &str = include_str!("../../../docs/optimization/STATE_SCHEMA_MIGRATION_POLICY.toml");
const CACHE: &str = include_str!("../../../docs/optimization/CACHE_REBUILD_INVALIDATION_POLICY.toml");
const EXERCISE: &str = include_str!("../../../docs/optimization/RECOVERY_EXERCISE_EVIDENCE_POLICY.toml");
const COVERAGE: &str =
    include_str!("../../../docs/optimization/END_TO_END_OPERATOR_STATE_RECOVERY_TRANCHE_COVERAGE.toml");

#[test]
fn operator_state_recovery_sources_are_registered() {
    for key in [
        "KUBERNETES_VOLUME_SNAPSHOTS",
        "ETCD_DISASTER_RECOVERY",
        "VELERO_BACKUP_REFERENCE",
        "VELERO_RESTORE_REFERENCE",
        "NIST_SP_800_34",
    ] {
        assert!(LEDGER.contains(key), "research source ledger must include {key}");
    }
}

#[test]
fn backup_restore_policy_records_scope_schedule_snapshot_integrity_restore_existing_resource_privacy_and_diagnostics() {
    for required in [
        "backup_id",
        "state_surface",
        "backup_scope_policy",
        "schedule_policy",
        "snapshot_policy",
        "integrity_policy",
        "restore_target_policy",
        "existing_resource_policy",
        "privacy_boundary",
        "kubernetes-operator-state-backup",
        "release-evidence-state-backup",
    ] {
        assert!(
            BACKUP.contains(required),
            "operator state backup restore policy must include {required}"
        );
    }
}

#[test]
fn state_schema_migration_policy_records_versions_compatibility_direction_idempotence_preflight_restore_evidence_and_diagnostics() {
    for required in [
        "migration_id",
        "state_surface",
        "schema_version_policy",
        "compatibility_policy",
        "migration_direction_policy",
        "idempotence_policy",
        "preflight_policy",
        "rollback_restore_policy",
        "evidence_policy",
        "operator-runtime-state-schema",
        "rule-data-and-scan-database-state",
    ] {
        assert!(
            MIGRATION.contains(required),
            "state schema migration policy must include {required}"
        );
    }
}

#[test]
fn cache_rebuild_policy_records_key_material_invalidation_rebuild_stale_read_artifact_identity_privacy_and_gate_effects() {
    for required in [
        "cache_id",
        "cache_surface",
        "key_material_policy",
        "invalidation_trigger_policy",
        "rebuild_policy",
        "stale_read_policy",
        "artifact_identity_policy",
        "privacy_boundary",
        "release_gate_effect",
        "compiled-pipeline-cache",
        "rule-and-scan-database-cache",
    ] {
        assert!(
            CACHE.contains(required),
            "cache rebuild invalidation policy must include {required}"
        );
    }
}

#[test]
fn recovery_exercise_policy_records_scenarios_source_state_restore_validation_slo_rollback_evidence_and_publication_boundary() {
    for required in [
        "exercise_id",
        "scenario_policy",
        "source_state_policy",
        "restore_action_policy",
        "validation_policy",
        "slo_policy",
        "rollback_policy",
        "evidence_capture_policy",
        "publication_boundary",
        "operator-state-restore-exercise",
        "cache-rebuild-after-restore-exercise",
    ] {
        assert!(
            EXERCISE.contains(required),
            "recovery exercise evidence policy must include {required}"
        );
    }
}

#[test]
fn plan_contains_operator_state_recovery_rows() {
    for row in [
        "VX-1281",
        "VX-1282",
        "VX-1283",
        "VX-1284",
        "VX-1285",
        "VX-1286",
        "VX-1287",
        "VX-1288",
        "VX-1289",
        "VX-1290",
        "VX-1291",
        "VX-1292",
        "VX-1293",
        "VX-1294",
        "VX-1295",
        "VX-1296",
        "VX-1297",
        "VX-1298",
        "VX-1299",
        "VX-1300",
    ] {
        assert!(PLAN.contains(row), "plan must include {row}");
    }
}

#[test]
fn operator_state_recovery_coverage_reuses_config_artifact_readiness_rollout_release_health_publication_and_dedup_authorities() {
    for required in [
        "VX-1281..VX-1300",
        "operator_state_backup_restore_policy",
        "state_schema_migration_policy",
        "cache_rebuild_invalidation_policy",
        "recovery_exercise_evidence_policy",
        "config_api_governance",
        "artifact_integrity_archive_coverage",
        "operational_readiness_coverage",
        "staged_rollout_coverage",
        "publication_boundary",
        "dedup_seam",
        "proof_gate",
    ] {
        assert!(
            COVERAGE.contains(required),
            "operator state recovery tranche coverage must include {required}"
        );
    }
}
