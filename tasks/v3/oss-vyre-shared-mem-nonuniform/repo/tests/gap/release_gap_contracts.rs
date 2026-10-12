// Integration test module for the containing Vyre package.

use std::fs;
use std::path::Path;

#[test]
fn release_gap_contracts_are_manifest_backed() {
    let manifest_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("release/vyre-weir-evidence.toml");
    let manifest = fs::read_to_string(&manifest_path)
        .expect("Fix: release evidence manifest must be readable for gap contract tests");

    for required in [
        "optimization-corpus-1000",
        "proof-workloads-10",
        "cpu-only-100x-proof",
        "c-parser-linux-subsystem",
        "conformance-hard-gate",
        "final-completion-audit",
    ] {
        assert!(
            manifest.contains(required),
            "release gap contract `{required}` must remain represented in release/vyre-weir-evidence.toml"
        );
    }
}
