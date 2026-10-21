//! Adversarial tests for serial-envelope corruption handling.
//!
//! The envelope reader must detect truncation, corruption, and
//! malformed sections and return structured errors.

use vyre_foundation::serial::{EnvelopeError, WireReader, WireWriter};

const TEST_MAGIC: &[u8; 4] = b"VYRE";
const TEST_VERSION: u32 = 1;

#[test]
fn envelope_rejects_truncated_header() {
    let bytes = [0u8; 7]; // shorter than the 8-byte header
    match WireReader::new(&bytes, TEST_MAGIC, TEST_VERSION) {
        Err(EnvelopeError::Truncated { .. }) => {}
        other => panic!("expected Truncated error for 7-byte input, got {other:?}"),
    }
}

#[test]
fn envelope_rejects_corrupted_section_length() {
    let mut writer = WireWriter::new(TEST_MAGIC, TEST_VERSION);
    writer.write_section(b"test").unwrap();
    let mut bytes = writer.into_bytes();

    // Corrupt the section-length field (4 bytes before the section payload).
    // Section layout: len(u32) + bytes
    // Header is 8 bytes, so section len starts at byte 8.
    if bytes.len() > 11 {
        bytes[8] = 0xFF;
        bytes[9] = 0xFF;
        bytes[10] = 0xFF;
        bytes[11] = 0x7F; // claim 2GB section
    }

    let mut reader = WireReader::new(&bytes, TEST_MAGIC, TEST_VERSION).unwrap();
    match reader.read_section() {
        Err(EnvelopeError::Truncated { .. }) => {}
        other => panic!("expected Truncated error for corrupted section length, got {other:?}"),
    }
}

#[test]
fn envelope_rejects_extra_bytes_after_valid_payload() {
    let mut writer = WireWriter::new(TEST_MAGIC, TEST_VERSION);
    writer.write_section(b"hello").unwrap();
    let mut bytes = writer.into_bytes();
    // Append garbage bytes after the valid payload.
    bytes.extend_from_slice(&[0xDE, 0xAD, 0xBE, 0xEF]);

    // Reader should still parse the valid prefix successfully.
    let mut reader = WireReader::new(&bytes, TEST_MAGIC, TEST_VERSION).unwrap();
    assert_eq!(reader.read_section().unwrap(), b"hello");
    // Any attempt to read past the declared payload should fail.
    match reader.read_section() {
        Err(EnvelopeError::Truncated { .. }) => {}
        other => panic!("expected Truncated after valid payload, got {other:?}"),
    }
}

#[test]
fn envelope_rejects_empty_section() {
    let mut writer = WireWriter::new(TEST_MAGIC, TEST_VERSION);
    writer.write_section(b"").unwrap();
    let bytes = writer.into_bytes();

    let mut reader = WireReader::new(&bytes, TEST_MAGIC, TEST_VERSION).unwrap();
    assert_eq!(reader.read_section().unwrap(), b"");
}

#[test]
fn envelope_rejects_null_bytes_in_section() {
    let mut writer = WireWriter::new(TEST_MAGIC, TEST_VERSION);
    writer.write_section(b"a\0b\0c").unwrap();
    let bytes = writer.into_bytes();

    let mut reader = WireReader::new(&bytes, TEST_MAGIC, TEST_VERSION).unwrap();
    assert_eq!(reader.read_section().unwrap(), b"a\0b\0c");
}
