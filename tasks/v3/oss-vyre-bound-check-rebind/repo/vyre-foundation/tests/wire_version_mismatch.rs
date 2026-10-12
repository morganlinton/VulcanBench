//! Adversarial tests for wire-format version handling.
//!
//! The wire decoder must reject programs encoded with a future or
//! past schema version, producing actionable errors rather than
//! silent misinterpretation or panics.

mod wire_decode_support;

use wire_decode_support::{decode_error_string, minimal_program_bytes};

#[test]
fn wire_decoder_rejects_future_version() {
    let mut bytes = minimal_program_bytes();
    // Wire format header: magic(4) + version(2) + flags(2) + checksum(32)
    // Version is at bytes[4..6] (little-endian u16).
    if bytes.len() >= 6 {
        bytes[4] = 0xFF;
        bytes[5] = 0x7F; // version 32767  -  far future
    }

    let error = decode_error_string(&bytes, "future wire schema version");
    assert_version_error(&error, "future version");
}

#[test]
fn wire_decoder_rejects_past_version() {
    let mut bytes = minimal_program_bytes();
    if bytes.len() >= 6 {
        bytes[4] = 0x00;
        bytes[5] = 0x00; // version 0  -  past/invalid
    }

    let error = decode_error_string(&bytes, "past wire schema version");
    assert_version_error(&error, "version 0");
}

fn assert_version_error(error: &str, label: &str) {
    assert!(
        error.contains("UnknownSchemaVersion")
            || error.contains("VersionMismatch")
            || error.contains("version")
            || error.contains("Fix:"),
        "{label} must produce a version error, got: {error}"
    );
}
