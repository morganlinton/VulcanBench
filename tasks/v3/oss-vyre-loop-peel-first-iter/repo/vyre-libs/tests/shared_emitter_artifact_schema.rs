//! Shared emitter artifact schema test suite.

const SCHEMA: &str = include_str!("../../docs/optimization/SHARED_EMITTER_ARTIFACT_SCHEMA.toml");

#[test]
fn shared_emitter_artifact_schema_unifies_hashes_and_abi_fields() {
    for required in [
        "msl",
        "wgsl",
        "spirv",
        "ptx",
        "descriptor_hash",
        "abi_metadata",
        "capability_digest",
        "source_map",
        "unsupported_features",
    ] {
        assert!(
            SCHEMA.contains(required),
            "shared emitter artifact schema must include {required}"
        );
    }
}
