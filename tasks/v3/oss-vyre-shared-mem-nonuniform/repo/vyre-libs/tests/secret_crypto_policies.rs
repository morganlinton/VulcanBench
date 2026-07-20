//! Secret crypto policies test suite.

const SECRETS: &str =
    include_str!("../../docs/optimization/SECRET_MATERIAL_HANDLING_POLICY.toml");
const CONSTANT_TIME: &str =
    include_str!("../../docs/optimization/CONSTANT_TIME_CRYPTO_CONTRACTS.toml");
const PASSWORDS: &str =
    include_str!("../../docs/optimization/PASSWORD_HASHING_DERIVATION_POLICY.toml");

#[test]
fn secret_material_policy_records_boundaries_access_logging_redaction_rotation_and_incidents() {
    for required in [
        "secret_class",
        "source_boundary",
        "storage_boundary",
        "access_policy",
        "logging_policy",
        "redaction_policy",
        "rotation_policy",
        "incident_policy",
        "diagnostic",
        "never-log-value",
    ] {
        assert!(
            SECRETS.contains(required),
            "secret material handling policy must include {required}"
        );
    }
}

#[test]
fn constant_time_contracts_record_secret_inputs_branch_memory_and_negative_tests() {
    for required in [
        "operation_id",
        "secret_inputs",
        "public_inputs",
        "comparison_policy",
        "branch_policy",
        "memory_access_policy",
        "negative_test",
        "diagnostic",
        "reject-early-return-byte-compare",
    ] {
        assert!(
            CONSTANT_TIME.contains(required),
            "constant-time crypto contract must include {required}"
        );
    }
}

#[test]
fn password_hashing_policy_records_argon2id_salt_parameters_storage_and_migration() {
    for required in [
        "derivation_id",
        "argon2id",
        "salt_policy",
        "parameter_floor",
        "pepper_policy",
        "storage_policy",
        "migration_policy",
        "diagnostic",
    ] {
        assert!(
            PASSWORDS.contains(required),
            "password hashing policy must include {required}"
        );
    }
}
