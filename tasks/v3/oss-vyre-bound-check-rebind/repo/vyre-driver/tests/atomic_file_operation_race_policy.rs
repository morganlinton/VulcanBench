//! Atomic file operation race policy test suite.

const RACE_POLICY: &str =
    include_str!("../../docs/optimization/ATOMIC_FILE_OPERATION_RACE_POLICY.toml");

#[test]
fn atomic_file_operation_policy_records_open_check_use_temp_rename_permission_and_race_diagnostics() {
    for required in [
        "operation_id",
        "target_class",
        "open_policy",
        "check_use_policy",
        "tempfile_policy",
        "atomic_promotion_policy",
        "permission_policy",
        "race_diagnostic",
        "check-and-use-same-open-handle",
        "reject-world-writable-ancestor",
    ] {
        assert!(
            RACE_POLICY.contains(required),
            "atomic file operation race policy must include {required}"
        );
    }
}
