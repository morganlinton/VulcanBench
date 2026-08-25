//! Release publication boundary test suite.

const MANIFEST: &str = include_str!("../../docs/optimization/PUBLICATION_BOUNDARY_MANIFEST.toml");

#[test]
fn publication_boundary_manifest_blocks_private_santh_material_from_public_vyre_artifacts() {
    for required in [
        "public_artifact_class",
        "private_artifact_class",
        "allowed_roots",
        "blocked_roots",
        "public-links-only",
        "reject-private-links",
        "VYRE_PUBLICATION_PRIVATE_BOUNDARY_REFUSED",
    ] {
        assert!(
            MANIFEST.contains(required),
            "publication boundary manifest must include {required}"
        );
    }
}
