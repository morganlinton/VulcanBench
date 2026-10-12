//! Executable contract tests for the self-contained generated artifact loader.

#![allow(unreachable_pub)]

#[path = "../templates/artifact.rs.tmpl"]
mod generated_loader;

use std::fs;
use std::path::Path;

use serde_json::json;
use sha2::{Digest, Sha256};

fn write_manifest(dir: &Path, manifest: serde_json::Value) {
    fs::write(
        dir.join("manifest.json"),
        serde_json::to_vec_pretty(&manifest).expect("manifest JSON must serialize"),
    )
    .expect("manifest write must succeed");
}

fn digest_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut out = String::with_capacity(digest.len() * 2);
    for byte in digest {
        let _ = std::fmt::Write::write_fmt(&mut out, format_args!("{byte:02x}"));
    }
    out
}

fn base_manifest() -> serde_json::Value {
    json!({
        "schema": "vyre-aot-manifest-v1",
        "aot_version": "0.4.2-test",
        "artifact_name": "generated-loader-contract",
        "target": "Ptx",
        "entry_point": "vyre_kernel",
        "dispatch": {
            "workgroup_size": [128, 1, 1],
            "grid_size": [8, 1, 1],
            "dynamic_shared_bytes": 0
        },
        "kernel_file": "kernel.ptx",
        "weights_file": "weights.bin",
        "kernel_compression": "none",
        "weights_compression": "none",
        "buffers": [
            {
                "name": "params",
                "binding": 0,
                "element_count": 16,
                "element_size_bytes": 4,
                "memory_kind": "Global",
                "access": "ReadOnly"
            },
            {
                "name": "metrics",
                "binding": 1,
                "element_count": 8,
                "element_size_bytes": 4,
                "memory_kind": "Global",
                "access": "ReadWrite"
            }
        ],
        "kernel_sha256_hex": "0000000000000000000000000000000000000000000000000000000000000000",
        "weights_sha256_hex": "0000000000000000000000000000000000000000000000000000000000000000",
        "vsa_fingerprint": [0, 1, 2, 3, 4, 5, 6, 7]
    })
}

fn load_error(manifest: serde_json::Value) -> String {
    let dir = tempfile::tempdir().expect("tempdir must be available");
    write_manifest(dir.path(), manifest);
    generated_loader::load_bundle(dir.path())
        .expect_err("manifest should be rejected")
        .to_string()
}

#[test]
fn generated_loader_rejects_schema_mismatch_before_payload_reads() {
    let mut manifest = base_manifest();
    manifest["schema"] = json!("vyre-aot-manifest-v0");

    let error = load_error(manifest);

    assert!(
        error.contains("unsupported manifest schema"),
        "expected schema error, got: {error}"
    );
    assert!(
        !error.contains("read bundle file"),
        "schema validation must run before payload reads, got: {error}"
    );
}

#[test]
fn generated_loader_rejects_runtime_grid_placeholder_before_payload_reads() {
    let mut manifest = base_manifest();
    manifest["dispatch"]["grid_size"] = json!([0, 1, 1]);

    let error = load_error(manifest);

    assert!(
        error.contains("runtime-grid placeholders are not supported"),
        "expected runtime-grid error, got: {error}"
    );
    assert!(
        !error.contains("read bundle file"),
        "dispatch validation must run before payload reads, got: {error}"
    );
}

#[test]
fn generated_loader_rejects_path_escape_before_payload_reads() {
    let mut manifest = base_manifest();
    manifest["kernel_file"] = json!("../kernel.ptx");

    let error = load_error(manifest);

    assert!(
        error.contains("escapes the bundle root"),
        "expected path escape error, got: {error}"
    );
    assert!(
        !error.contains("read bundle file"),
        "path validation must fail before filesystem payload reads, got: {error}"
    );
}

#[test]
fn generated_loader_rejects_duplicate_bindings_before_payload_reads() {
    let mut manifest = base_manifest();
    manifest["buffers"][1]["binding"] = json!(0);

    let error = load_error(manifest);

    assert!(
        error.contains("both use binding"),
        "expected duplicate binding error, got: {error}"
    );
    assert!(
        !error.contains("read bundle file"),
        "buffer ABI validation must run before payload reads, got: {error}"
    );
}

#[test]
fn generated_loader_accepts_valid_raw_payload_bundle_and_verifies_hashes() {
    let dir = tempfile::tempdir().expect("tempdir must be available");
    let kernel = b".version 8.9\n.visible .entry vyre_kernel() { ret; }\n";
    let weights = [1_u8, 2, 3, 5, 8, 13, 21, 34];
    fs::write(dir.path().join("kernel.ptx"), kernel).expect("kernel write must succeed");
    fs::write(dir.path().join("weights.bin"), weights).expect("weights write must succeed");

    let mut manifest = base_manifest();
    manifest["kernel_sha256_hex"] = json!(digest_hex(kernel));
    manifest["weights_sha256_hex"] = json!(digest_hex(&weights));
    write_manifest(dir.path(), manifest);

    let loaded = generated_loader::load_bundle(dir.path()).expect("valid bundle must load");

    assert_eq!(loaded.kernel_bytes, kernel);
    assert_eq!(loaded.weight_bytes, weights);
    assert_eq!(loaded.manifest.entry_point, "vyre_kernel");
}
