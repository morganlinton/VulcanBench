//! Tests for device signature paths.

use std::fs;
use std::path::PathBuf;
use vyre_driver::DeviceSignature;

#[test]
fn blackwell_120_signature_can_be_loaded() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let path = manifest_dir.join("../devices/blackwell_120.toml");

    assert!(
        path.exists(),
        "blackwell_120.toml is missing from devices directory"
    );

    let source = fs::read_to_string(&path).expect("could not read blackwell_120.toml");

    let signature =
        DeviceSignature::from_toml_str(&source).expect("could not parse blackwell_120.toml");

    assert_eq!(signature.id, "blackwell_120");
}
