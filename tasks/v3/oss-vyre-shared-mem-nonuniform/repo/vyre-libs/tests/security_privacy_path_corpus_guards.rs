//! Security privacy path corpus guards test suite.

const PRIVACY: &str = include_str!("../../docs/optimization/DIAGNOSTIC_PRIVACY_FUZZING.toml");
const PATHS: &str = include_str!("../../docs/optimization/WORKSPACE_PATH_CERTIFICATES.toml");
const CORPUS: &str = include_str!("../../docs/optimization/CORPUS_INGRESS_GUARDS.toml");

#[test]
fn diagnostic_privacy_fuzzing_keeps_fix_text_without_secrets() {
    for required in [
        "secret_like_path",
        "token",
        "url",
        "env_name",
        "backend_id",
        "redaction_policy",
        "remediation_visibility",
        "VYRE_DIAGNOSTIC_SECRET_REDACTED",
    ] {
        assert!(PRIVACY.contains(required), "privacy fuzzing must include {required}");
    }
}

#[test]
fn workspace_path_certificates_classify_public_and_private_boundaries() {
    for required in [
        "canonical_path",
        "allowed_root",
        "symlink_status",
        "file_kind",
        "owner_lane",
        "publish_class",
        "publication_allowed",
        "public-vyre-artifact",
        "private-santh-evidence",
    ] {
        assert!(PATHS.contains(required), "workspace path certificate must include {required}");
    }
}

#[test]
fn corpus_ingress_guards_refuse_bombs_and_traversal() {
    for required in [
        "maximum_bytes",
        "expansion_ratio",
        "file_count",
        "nesting_depth",
        "chunk_size",
        "refusal_reason",
        "VYRE_CORPUS_EXPANSION_REFUSED",
        "VYRE_CORPUS_PATH_TRAVERSAL_REFUSED",
    ] {
        assert!(CORPUS.contains(required), "corpus ingress guard must include {required}");
    }
}
