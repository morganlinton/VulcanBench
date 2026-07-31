//! P1 inventory #97  -  AOT/runtime artifact round-trip test.
//!
//! The AOT bundle's `Manifest` is the single JSON file the launcher
//! reads at startup. Round-tripping it through serde must produce a
//! byte-identical structure so that consumers can verify the bundle
//! survived disk → upload → distribution → run-time decode without
//! losing any field.
//!
//! Paired with `tests/compile_smoke.rs` which exercises the full
//! compile-emit path; this test owns the manifest schema contract.

use vyre_aot::artifact::{BufferAccessKind, BufferEntry, BufferMemoryKind, DispatchConfig, Target};
use vyre_aot::manifest::Manifest;

#[test]
fn manifest_round_trips_through_serde_json() {
    let original = Manifest {
        schema: Manifest::SCHEMA_VERSION.to_string(),
        aot_version: "0.6.0".to_string(),
        artifact_name: "test-artifact".to_string(),
        target: Target::SpirV,
        entry_point: "main".to_string(),
        dispatch: DispatchConfig {
            workgroup_size: [64, 1, 1],
            grid_size: [1, 1, 1],
            dynamic_shared_bytes: 0,
        },
        kernel_file: "kernel.spv.lzma".to_string(),
        weights_file: "weights.bin.brotli".to_string(),
        kernel_compression: "lzma".to_string(),
        weights_compression: "brotli-11".to_string(),
        buffers: vec![BufferEntry {
            name: "out".to_string(),
            binding: 0,
            element_count: 64,
            element_size_bytes: 4,
            memory_kind: BufferMemoryKind::Global,
            access: BufferAccessKind::WriteOnly,
        }],
        kernel_sha256_hex: "deadbeef".repeat(8),
        weights_sha256_hex: "cafebabe".repeat(8),
        notes: "smoke notes".to_string(),
        vsa_fingerprint: vec![1, 2, 3, 4, 5, 6, 7, 8],
    };

    // Round trip through JSON.
    let bytes = serde_json::to_vec_pretty(&original).expect("manifest serialization failed");
    let restored: Manifest =
        serde_json::from_slice(&bytes).expect("manifest deserialization failed");

    assert_eq!(restored.schema, original.schema);
    assert_eq!(restored.aot_version, original.aot_version);
    assert_eq!(restored.artifact_name, original.artifact_name);
    assert_eq!(restored.target, original.target);
    assert_eq!(restored.entry_point, original.entry_point);
    assert_eq!(
        restored.dispatch.workgroup_size,
        original.dispatch.workgroup_size
    );
    assert_eq!(restored.dispatch.grid_size, original.dispatch.grid_size);
    assert_eq!(
        restored.dispatch.dynamic_shared_bytes,
        original.dispatch.dynamic_shared_bytes
    );
    assert_eq!(restored.buffers.len(), original.buffers.len());
    assert_eq!(restored.kernel_sha256_hex, original.kernel_sha256_hex);
    assert_eq!(restored.weights_sha256_hex, original.weights_sha256_hex);
    assert_eq!(restored.vsa_fingerprint, original.vsa_fingerprint);
}

#[test]
fn manifest_default_optional_fields_round_trip() {
    // notes + vsa_fingerprint are `#[serde(default)]`; a manifest
    // missing them must deserialize successfully with empty values.
    let json = format!(
        r#"{{
            "schema": "{}",
            "aot_version": "0.6.0",
            "artifact_name": "minimal",
            "target": "Ptx",
            "entry_point": "main",
            "dispatch": {{"workgroup_size":[1,1,1],"grid_size":[1,1,1],"dynamic_shared_bytes":0}},
            "kernel_file": "k.secondary_text.lzma",
            "weights_file": "w.bin.brotli",
            "kernel_compression": "lzma",
            "weights_compression": "brotli-11",
            "buffers": [],
            "kernel_sha256_hex": "00",
            "weights_sha256_hex": "00"
        }}"#,
        Manifest::SCHEMA_VERSION
    );
    let m: Manifest = serde_json::from_str(&json).expect("minimal manifest must parse");
    assert!(m.notes.is_empty());
    assert!(m.vsa_fingerprint.is_empty());
}
