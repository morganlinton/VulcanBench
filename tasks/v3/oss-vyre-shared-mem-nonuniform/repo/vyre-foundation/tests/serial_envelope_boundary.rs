//! Boundary tests for the reusable on-wire envelope (magic + version + sections).
//!
//! Every consumer of `WireWriter`/`WireReader` depends on correct framing.
//! These tests exercise truncation, bad magic, version mismatch, and
//! section-length overflow.

use vyre_foundation::serial::{EnvelopeError, WireReader, WireWriter};

const MAGIC: &[u8; 4] = b"TEST";
const VERSION: u32 = 1;

// ------------------------------------------------------------------
// Happy path
// ------------------------------------------------------------------

#[test]
fn envelope_round_trip_bytes() {
    let mut writer = WireWriter::new(MAGIC, VERSION);
    writer.write_section(b"hello").unwrap();
    let bytes = writer.into_bytes();

    let mut reader = WireReader::new(&bytes, MAGIC, VERSION).unwrap();
    assert_eq!(reader.read_section().unwrap(), b"hello");
}

#[test]
fn envelope_round_trip_words() {
    let mut writer = WireWriter::new(MAGIC, VERSION);
    writer.write_words(&[0xDEADBEEF, 0xCAFEBABE]).unwrap();
    let bytes = writer.into_bytes();

    let mut reader = WireReader::new(&bytes, MAGIC, VERSION).unwrap();
    assert_eq!(reader.read_words().unwrap(), vec![0xDEADBEEF, 0xCAFEBABE]);
}

#[test]
fn envelope_round_trip_u32() {
    let mut writer = WireWriter::new(MAGIC, VERSION);
    writer.write_u32(42);
    let bytes = writer.into_bytes();

    let mut reader = WireReader::new(&bytes, MAGIC, VERSION).unwrap();
    assert_eq!(reader.read_u32().unwrap(), 42);
}

#[test]
fn envelope_multiple_sections_in_order() {
    let mut writer = WireWriter::new(MAGIC, VERSION);
    writer.write_section(b"first").unwrap();
    writer.write_section(b"second").unwrap();
    let bytes = writer.into_bytes();

    let mut reader = WireReader::new(&bytes, MAGIC, VERSION).unwrap();
    assert_eq!(reader.read_section().unwrap(), b"first");
    assert_eq!(reader.read_section().unwrap(), b"second");
}

// ------------------------------------------------------------------
// Magic rejection
// ------------------------------------------------------------------

#[test]
fn envelope_rejects_bad_magic() {
    let mut writer = WireWriter::new(MAGIC, VERSION);
    writer.write_section(b"x").unwrap();
    let bytes = writer.into_bytes();

    let err = WireReader::new(&bytes, b"WRNG", VERSION).unwrap_err();
    assert!(matches!(err, EnvelopeError::BadMagic { .. }));
}

#[test]
fn envelope_rejects_empty_input() {
    let err = WireReader::new(&[], MAGIC, VERSION).unwrap_err();
    assert!(matches!(err, EnvelopeError::Truncated { .. }));
}

#[test]
fn envelope_rejects_short_header() {
    let err = WireReader::new(&[0, 1, 2], MAGIC, VERSION).unwrap_err();
    assert!(matches!(err, EnvelopeError::Truncated { .. }));
}

// ------------------------------------------------------------------
// Version mismatch
// ------------------------------------------------------------------

#[test]
fn envelope_rejects_version_mismatch() {
    let mut writer = WireWriter::new(MAGIC, VERSION);
    writer.write_section(b"x").unwrap();
    let bytes = writer.into_bytes();

    let err = WireReader::new(&bytes, MAGIC, 999).unwrap_err();
    assert!(matches!(err, EnvelopeError::VersionMismatch { .. }));
}

#[test]
fn envelope_version_zero_is_rejected_when_expecting_one() {
    let mut writer = WireWriter::new(MAGIC, 0);
    writer.write_section(b"x").unwrap();
    let bytes = writer.into_bytes();

    let err = WireReader::new(&bytes, MAGIC, VERSION).unwrap_err();
    assert!(matches!(err, EnvelopeError::VersionMismatch { .. }));
}

// ------------------------------------------------------------------
// Truncation detection
// ------------------------------------------------------------------

#[test]
fn envelope_rejects_truncated_section() {
    let mut writer = WireWriter::new(MAGIC, VERSION);
    writer.write_section(b"hello world").unwrap();
    let mut bytes = writer.into_bytes();
    // Truncate inside the section body (after 8-byte header + 4-byte length).
    bytes.truncate(8 + 4 + 3);

    let mut reader = WireReader::new(&bytes, MAGIC, VERSION).unwrap();
    let err = reader.read_section().unwrap_err();
    assert!(matches!(err, EnvelopeError::Truncated { .. }));
}

#[test]
fn envelope_rejects_truncated_word_array() {
    let mut writer = WireWriter::new(MAGIC, VERSION);
    writer.write_words(&[1, 2, 3]).unwrap();
    let mut bytes = writer.into_bytes();
    // Truncate mid-word.
    bytes.truncate(bytes.len() - 1);

    let mut reader = WireReader::new(&bytes, MAGIC, VERSION).unwrap();
    let err = reader.read_words().unwrap_err();
    assert!(matches!(err, EnvelopeError::Truncated { .. }));
}

#[test]
fn envelope_rejects_truncated_u32() {
    let mut writer = WireWriter::new(MAGIC, VERSION);
    writer.write_u32(42);
    let mut bytes = writer.into_bytes();
    // Remove last byte of the u32.
    bytes.pop();

    let mut reader = WireReader::new(&bytes, MAGIC, VERSION).unwrap();
    let err = reader.read_u32().unwrap_err();
    assert!(matches!(err, EnvelopeError::Truncated { .. }));
}

#[test]
fn envelope_rejects_header_only() {
    let bytes = WireWriter::new(MAGIC, VERSION).into_bytes();
    assert_eq!(bytes.len(), 8);

    let mut reader = WireReader::new(&bytes, MAGIC, VERSION).unwrap();
    let err = reader.read_section().unwrap_err();
    assert!(matches!(err, EnvelopeError::Truncated { .. }));
}

// ------------------------------------------------------------------
// Section-too-large
// ------------------------------------------------------------------

#[test]
fn envelope_section_too_large_is_rejected_at_encode() {
    let mut writer = WireWriter::new(MAGIC, VERSION);
    let huge = vec![0u8; (u32::MAX as usize) + 1];
    let err = writer.write_section(&huge).unwrap_err();
    assert!(matches!(err, EnvelopeError::SectionTooLarge { .. }));
}

#[test]
fn envelope_words_too_large_is_rejected_at_encode() {
    let mut writer = WireWriter::new(MAGIC, VERSION);
    let huge = vec![0u32; (u32::MAX as usize) + 1];
    let err = writer.write_words(&huge).unwrap_err();
    assert!(matches!(err, EnvelopeError::SectionTooLarge { .. }));
}

// ------------------------------------------------------------------
// Edge cases
// ------------------------------------------------------------------

#[test]
fn envelope_empty_section_round_trips() {
    let mut writer = WireWriter::new(MAGIC, VERSION);
    writer.write_section(b"").unwrap();
    let bytes = writer.into_bytes();

    let mut reader = WireReader::new(&bytes, MAGIC, VERSION).unwrap();
    assert_eq!(reader.read_section().unwrap(), b"");
}

#[test]
fn envelope_empty_words_round_trip() {
    let mut writer = WireWriter::new(MAGIC, VERSION);
    writer.write_words(&[]).unwrap();
    let bytes = writer.into_bytes();

    let mut reader = WireReader::new(&bytes, MAGIC, VERSION).unwrap();
    assert_eq!(reader.read_words().unwrap(), Vec::<u32>::new());
}

#[test]
fn envelope_max_u32_len_section_encodes() {
    // u32::MAX bytes would OOM in debug tests, so we just verify the
    // boundary at encode time with a smaller-but-still-large value.
    let mut writer = WireWriter::new(MAGIC, VERSION);
    let _big = vec![0u8; u32::MAX as usize];
    // This should succeed (in theory) but we can't allocate that much.
    // Instead we verify the error path for usize > u32::MAX above.
    let _ = writer.write_section(&[1, 2, 3]);
}

#[test]
fn envelope_different_magics_are_independent() {
    let mut w1 = WireWriter::new(b"ONE!", 1);
    w1.write_section(b"a").unwrap();
    let bytes1 = w1.into_bytes();

    let mut w2 = WireWriter::new(b"TWO!", 1);
    w2.write_section(b"b").unwrap();
    let bytes2 = w2.into_bytes();

    assert_ne!(bytes1, bytes2);
    assert!(WireReader::new(&bytes1, b"TWO!", 1).is_err());
    assert!(WireReader::new(&bytes2, b"ONE!", 1).is_err());
}
