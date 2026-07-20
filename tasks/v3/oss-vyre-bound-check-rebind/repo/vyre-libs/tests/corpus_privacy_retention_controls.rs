//! Corpus privacy retention controls test suite.

const PRIVACY: &str =
    include_str!("../../docs/optimization/CORPUS_PRIVACY_CLASSIFICATION.toml");
const RETENTION: &str =
    include_str!("../../docs/optimization/DATA_RETENTION_MEDIA_CONTROLS.toml");

#[test]
fn corpus_privacy_classification_records_risk_allowed_use_redaction_and_publication_gates() {
    for required in [
        "classification_id",
        "data_category",
        "privacy_risk",
        "allowed_use",
        "redaction_policy",
        "aggregation_policy",
        "publication_gate",
        "operator_diagnostic",
        "blocked-from-publication",
    ] {
        assert!(
            PRIVACY.contains(required),
            "corpus privacy classification must include {required}"
        );
    }
}

#[test]
fn data_retention_media_controls_record_storage_deletion_quarantine_and_transfer_policy() {
    for required in [
        "control_id",
        "data_class",
        "storage_scope",
        "retention_policy",
        "deletion_policy",
        "quarantine_policy",
        "transfer_policy",
        "audit_evidence",
        "no-publication-no-package-no-release-artifact",
    ] {
        assert!(
            RETENTION.contains(required),
            "data retention media controls must include {required}"
        );
    }
}
