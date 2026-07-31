//! Filesystem path archive policies test suite.

const PATH_POLICY: &str =
    include_str!("../../docs/optimization/FILESYSTEM_PATH_CANONICALIZATION_POLICY.toml");
const ARCHIVE_GUARDS: &str =
    include_str!("../../docs/optimization/ARCHIVE_EXTRACTION_BOMB_GUARDS.toml");

#[test]
fn filesystem_path_policy_records_allowed_roots_canonicalization_symlinks_and_escape_checks() {
    for required in [
        "policy_id",
        "input_class",
        "allowed_root",
        "canonicalization_order",
        "symlink_policy",
        "file_kind_policy",
        "escape_check",
        "diagnostic",
        "reject-symlink-escape",
    ] {
        assert!(
            PATH_POLICY.contains(required),
            "filesystem path policy must include {required}"
        );
    }
}

#[test]
fn archive_extraction_guards_record_member_paths_limits_nested_policy_and_roots() {
    for required in [
        "guard_id",
        "archive_format",
        "member_path_policy",
        "compression_ratio_limit",
        "expanded_bytes_limit",
        "file_count_limit",
        "nested_archive_policy",
        "extraction_root_policy",
        "VYRE_ARCHIVE_BOMB_REFUSED",
    ] {
        assert!(
            ARCHIVE_GUARDS.contains(required),
            "archive extraction guard must include {required}"
        );
    }
}
