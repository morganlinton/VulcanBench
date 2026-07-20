//! Adversarial tests for wire-format corruption detection.
//!
//! The wire decoder must reject tampered or corrupted payloads with
//! structured errors rather than panics or silent acceptance.

mod wire_decode_support;

use wire_decode_support::{decode_error_string, minimal_program_bytes};

#[test]
fn wire_decoder_rejects_corrupted_checksum() {
    let mut bytes = minimal_program_bytes();
    // Corrupt a single byte in the body (after the 40-byte header).
    // The header contains: magic(4) + version(2) + flags(2) + checksum(32) = 40 bytes.
    if bytes.len() > 45 {
        bytes[45] = bytes[45].wrapping_add(1);
    }

    let error = decode_error_string(&bytes, "corrupt checksum");
    assert!(
        error.contains("IntegrityMismatch") || error.contains("Fix:"),
        "corrupt checksum must produce IntegrityMismatch or actionable error, got: {error}"
    );
}

#[test]
fn wire_decoder_rejects_truncated_body() {
    let bytes = minimal_program_bytes();
    // Truncate the body but leave the header intact  -  the checksum
    // will still be valid for a shorter body, but the decoder should
    // hit EOF before finishing node parsing.
    let truncated = &bytes[..bytes.len().saturating_sub(4)];

    decode_error_string(truncated, "truncated body");
}

#[test]
fn wire_decoder_rejects_wrong_magic() {
    let mut bytes = minimal_program_bytes();
    // Corrupt the magic bytes at the start.
    if bytes.len() >= 4 {
        bytes[0] = b'X';
        bytes[1] = b'X';
        bytes[2] = b'X';
        bytes[3] = b'X';
    }

    let error = decode_error_string(&bytes, "wrong magic");
    assert!(
        error.contains("MagicMismatch") || error.contains("Fix:"),
        "wrong magic must produce MagicMismatch or actionable error, got: {error}"
    );
}

#[test]
fn wire_decoder_rejects_empty_input() {
    decode_error_string(&[], "empty input");
}

#[test]
fn wire_decoder_rejects_header_only() {
    let bytes = minimal_program_bytes();
    // Keep only the 40-byte header, drop all body bytes.
    let header_only = &bytes[..40.min(bytes.len())];

    decode_error_string(header_only, "header-only input");
}
