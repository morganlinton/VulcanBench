//! Dataset lineage catalog test suite.

const CATALOG: &str = include_str!("../../docs/optimization/DATASET_LINEAGE_CATALOG.toml");

#[test]
fn dataset_lineage_catalog_records_catalog_provenance_distribution_and_publication_fields() {
    for required in [
        "dataset_id",
        "catalog_entry",
        "entity_digest",
        "activity",
        "agent",
        "distribution",
        "license_class",
        "access_rights",
        "derived_from",
        "publication_class",
    ] {
        assert!(
            CATALOG.contains(required),
            "dataset lineage catalog must include {required}"
        );
    }
}
